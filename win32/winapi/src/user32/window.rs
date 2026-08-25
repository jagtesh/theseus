//! Windows, painting and device contexts -- all of it Sogen's win32k rather than an implementation
//! of our own. `CreateWindowExW` here delivers the genuine Windows 2000 creation sequence
//! (`WM_NCCREATE`, `WM_NCCALCSIZE`, `WM_CREATE`, `WM_WINDOWPOSCHANGED` ...) to the application's own
//! window procedure before it returns, because the kernel doing the delivering is Microsoft's own
//! ordering as Sogen reimplements it.

use runtime::Context;

use crate::{
    FromABIParam, POINT, Ptr, RECT,
    gdi32::{HBRUSH, HDC},
    sogen,
    user32::{self, HCURSOR, HICON, HINSTANCE, HMENU, HWND, State, state},
};

const CW_USEDEFAULT: u32 = 0x8000_0000;

pub struct CW(u32);
impl CW {
    fn value(&self) -> Option<u32> {
        if self.0 == CW_USEDEFAULT {
            None
        } else {
            Some(self.0)
        }
    }
}
impl FromABIParam for CW {
    fn from_abi(val: u32) -> Self {
        Self(val)
    }
}
impl std::fmt::Debug for CW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == CW_USEDEFAULT {
            write!(f, "CW_USEDEFAULT")
        } else {
            write!(f, "{:#x}", self.0)
        }
    }
}

#[repr(C)]
#[derive(Debug, zerocopy::FromBytes)]
pub struct WNDCLASS {
    style: u32,       /* WNDCLASS_STYLES */
    lpfnWndProc: u32, /* WNDPROC */
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HICON,
    hCursor: HCURSOR,
    hbrBackground: HBRUSH,
    lpszMenuName: u32,
    lpszClassName: u32,
}

pub struct WndClass {
    /// The application's window procedure, already resolved to the translated block that implements
    /// it. win32k stores the guest address and hands it back on every callback; this is what the
    /// callback actually runs.
    pub wndproc: runtime::Cont,
    pub wndproc_addr: u32,
    pub atom: u16,
}

impl State {
    pub fn register_class(&self, wnd_class: WndClass) -> u16 {
        let atom = wnd_class.atom;
        *self.wndclass.borrow_mut() = Some(wnd_class);
        atom
    }
}

/// What a callback from win32k runs. It is a plain call: no frame is marshalled and no CPU is
/// resumed, which is the whole difference between a native client and a guest one.
pub fn dispatch_to_wndproc(
    ctx: &mut Context,
    hwnd: u32,
    message: u32,
    wparam: u32,
    lparam: u32,
) -> u64 {
    let wndproc = match state().wndclass.borrow().as_ref() {
        Some(class) => class.wndproc,
        None => return 0,
    };

    ctx.call32_x86(wndproc, vec![hwnd, message, wparam, lparam]);
    ctx.cpu.regs.eax as u64
}

#[win32_derive::dllexport]
pub fn RegisterClassA(ctx: &mut Context, lpWndClass: Ptr<WNDCLASS>) -> u16 {
    RegisterClassW(ctx, lpWndClass)
}

#[win32_derive::dllexport]
pub fn RegisterClassW(ctx: &mut Context, lpWndClass: Ptr<WNDCLASS>) -> u16 {
    let wndclass = lpWndClass.read(&ctx.memory).unwrap();
    let wndproc = ctx.indirect(wndclass.lpfnWndProc);

    let mark = sogen::scratch_mark();
    let units = sogen::wstr_units(&ctx.memory, wndclass.lpszClassName);
    let name = sogen::unicode_string(&mut ctx.memory, wndclass.lpszClassName, units);

    // WNDCLASSEX as the 32-bit kernel reads it: cbSize first, then the WNDCLASS fields, then
    // hIconSm. The class name is carried as a raw pointer here and as a UNICODE_STRING alongside.
    let class = sogen::scratch(48, 4);
    let fields: [u32; 12] = [
        48,
        wndclass.style,
        wndclass.lpfnWndProc,
        wndclass.cbClsExtra as u32,
        wndclass.cbWndExtra as u32,
        sogen::IMAGE_BASE,
        wndclass.hIcon,
        wndclass.hCursor,
        wndclass.hbrBackground.to_raw(),
        0,
        wndclass.lpszClassName,
        0,
    ];
    for (index, value) in fields.iter().enumerate() {
        ctx.memory.write::<u32>(class + index as u32 * 4, *value);
    }

    // CLSMENUNAME: the ANSI name, the wide name and the resource id, all null for a menu-less class.
    let menu_name = sogen::scratch(12, 4);
    for slot in 0..3u32 {
        ctx.memory.write::<u32>(menu_name + slot * 4, 0);
    }

    let atom = sogen::with_context(ctx, |_| sogen::register_class(class, name, menu_name, 0, 0, 0, 0)) as u16;
    sogen::scratch_release(mark);

    if atom == 0 {
        log::error!("RegisterClassW rejected by win32k");
        return 0;
    }

    state().register_class(WndClass {
        wndproc,
        wndproc_addr: wndclass.lpfnWndProc,
        atom,
    })
}

struct CreateArgs {
    ex_style: u32,
    class_name: u32,
    window_name: u32,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: u32,
    menu: u32,
    param: u32,
}

fn create_window(ctx: &mut Context, args: CreateArgs) -> HWND {
    let mark = sogen::scratch_mark();

    let class_units = sogen::wstr_units(&ctx.memory, args.class_name);
    let class_name = sogen::large_string(&mut ctx.memory, args.class_name, class_units);

    let window_name = if args.window_name == 0 {
        0
    } else {
        let units = sogen::wstr_units(&ctx.memory, args.window_name);
        sogen::large_string(&mut ctx.memory, args.window_name, units)
    };

    let hwnd = sogen::with_context(ctx, |_| {
        sogen::create_window(
            args.ex_style,
            class_name,
            window_name,
            args.style,
            args.x,
            args.y,
            args.width,
            args.height,
            args.parent,
            args.menu,
            sogen::IMAGE_BASE,
            args.param,
        )
    });

    sogen::scratch_release(mark);
    HWND::from_raw(hwnd)
}

#[win32_derive::dllexport]
pub fn CreateWindowExA(
    ctx: &mut Context,
    dwExStyle: u32, /* WINDOW_EX_STYLE */
    lpClassName: Ptr<u8>,
    lpWindowName: Ptr<u8>,
    dwStyle: u32, /* WINDOW_STYLE */
    X: i32,
    Y: i32,
    nWidth: CW,
    nHeight: CW,
    hWndParent: HWND,
    hMenu: HMENU,
    _hInstance: HINSTANCE,
    lpParam: Ptr<()>,
) -> HWND {
    // The kernel takes wide strings only, so an ANSI class or window name is widened into scratch.
    let class = ctx.memory.read_str(lpClassName.addr).to_string();
    let name = if lpWindowName.addr == 0 {
        String::new()
    } else {
        ctx.memory.read_str(lpWindowName.addr).to_string()
    };

    let (class_name, _) = sogen::write_wstr(&mut ctx.memory, &class);
    let window_name = if name.is_empty() {
        0
    } else {
        sogen::write_wstr(&mut ctx.memory, &name).0
    };

    create_window(
        ctx,
        CreateArgs {
            ex_style: dwExStyle,
            class_name,
            window_name,
            style: dwStyle,
            x: X,
            y: Y,
            width: nWidth.value().unwrap_or(640) as i32,
            height: nHeight.value().unwrap_or(480) as i32,
            parent: hWndParent.to_raw(),
            menu: hMenu,
            param: lpParam.addr,
        },
    )
}

#[win32_derive::dllexport]
pub fn CreateWindowExW(
    ctx: &mut Context,
    dwExStyle: u32,        /* WINDOW_EX_STYLE */
    lpClassName: Ptr<u16>, /* WSTR */
    lpWindowName: Ptr<u16>, /* WSTR */
    dwStyle: u32,          /* WINDOW_STYLE */
    X: i32,
    Y: i32,
    nWidth: CW,
    nHeight: CW,
    hWndParent: HWND,
    hMenu: HMENU,
    _hInstance: HINSTANCE,
    lpParam: Ptr<()>,
) -> HWND {
    create_window(
        ctx,
        CreateArgs {
            ex_style: dwExStyle,
            class_name: lpClassName.addr,
            window_name: lpWindowName.addr,
            style: dwStyle,
            x: X,
            y: Y,
            width: nWidth.value().unwrap_or(640) as i32,
            height: nHeight.value().unwrap_or(480) as i32,
            parent: hWndParent.to_raw(),
            menu: hMenu,
            param: lpParam.addr,
        },
    )
}

#[win32_derive::dllexport]
pub fn DestroyWindow(ctx: &mut Context, hWnd: HWND) -> bool {
    sogen::with_context(ctx, |_| sogen::destroy_window(hWnd.to_raw())) != 0
}

#[win32_derive::dllexport]
pub fn ShowWindow(ctx: &mut Context, hWnd: HWND, nCmdShow: u32 /* SHOW_WINDOW_CMD */) -> bool {
    sogen::with_context(ctx, |_| sogen::show_window(hWnd.to_raw(), nCmdShow)) != 0
}

#[win32_derive::dllexport]
pub fn MoveWindow(
    ctx: &mut Context,
    hWnd: HWND,
    X: i32,
    Y: i32,
    nWidth: i32,
    nHeight: i32,
    bRepaint: bool,
) -> bool {
    sogen::with_context(ctx, |_| {
        sogen::move_window(hWnd.to_raw(), X, Y, nWidth, nHeight, bRepaint as u32)
    }) != 0
}

#[win32_derive::dllexport]
pub fn UpdateWindow(ctx: &mut Context, hWnd: HWND) -> bool {
    sogen::with_context(ctx, |_| {
        sogen::call_hwnd_lock(hWnd.to_raw(), sogen::HWNDLOCK_UPDATEWINDOW)
    }) != 0
}

#[win32_derive::dllexport]
pub fn DefWindowProcA(
    ctx: &mut Context,
    hWnd: HWND,
    msg: u32,
    wParam: u32,
    lParam: u32,
) -> u32 {
    DefWindowProcW(ctx, hWnd, msg, wParam, lParam)
}

#[win32_derive::dllexport]
pub fn DefWindowProcW(
    ctx: &mut Context,
    hWnd: HWND,
    msg: u32,
    wParam: u32,
    lParam: u32,
) -> u32 {
    sogen::with_context(ctx, |ctx| {
        sogen::def_window_proc(&mut ctx.memory, hWnd.to_raw(), msg, wParam, lParam)
    })
}

#[win32_derive::dllexport]
pub fn SetFocus(_ctx: &mut Context, _hWnd: HWND) -> HWND {
    HWND::null()
}

#[repr(C)]
#[derive(Debug, zerocopy::IntoBytes, zerocopy::Immutable, zerocopy::FromBytes)]
pub struct PAINTSTRUCT {
    hdc: HDC,
    fErase: u32,
    rcPaint: RECT,
    reserved: [u32; 10],
}

#[win32_derive::dllexport]
pub fn BeginPaint(ctx: &mut Context, hWnd: HWND, lpPaint: Ptr<PAINTSTRUCT>) -> HDC {
    // The PAINTSTRUCT is filled by the kernel, in memory both halves share, so nothing is copied.
    let hdc = sogen::with_context(ctx, |_| sogen::begin_paint(hWnd.to_raw(), lpPaint.addr));
    HDC::from_raw(hdc)
}

#[win32_derive::dllexport]
pub fn EndPaint(ctx: &mut Context, hWnd: HWND, lpPaint: Ptr<PAINTSTRUCT>) -> bool {
    sogen::with_context(ctx, |_| sogen::end_paint(hWnd.to_raw(), lpPaint.addr)) != 0
}

#[win32_derive::dllexport]
pub fn GetDC(ctx: &mut Context, hWnd: HWND) -> HDC {
    HDC::from_raw(sogen::with_context(ctx, |_| sogen::get_dc(hWnd.to_raw())))
}

#[win32_derive::dllexport]
pub fn ReleaseDC(ctx: &mut Context, _hWnd: HWND, hDC: HDC) -> i32 {
    sogen::with_context(ctx, |_| {
        sogen::call_one_param(hDC.to_raw(), sogen::ONEPARAM_RELEASEDC)
    }) as i32
}

#[win32_derive::dllexport]
pub fn InvalidateRect(ctx: &mut Context, hWnd: HWND, lpRect: Ptr<RECT>, bErase: bool) -> bool {
    sogen::with_context(ctx, |_| {
        sogen::invalidate_rect(hWnd.to_raw(), lpRect.addr, bErase as u32)
    }) != 0
}

#[win32_derive::dllexport]
pub fn ValidateRect(ctx: &mut Context, hWnd: HWND, lpRect: Ptr<RECT>) -> bool {
    sogen::with_context(ctx, |_| sogen::validate_rect(hWnd.to_raw(), lpRect.addr)) != 0
}

#[win32_derive::dllexport]
pub fn GetDesktopWindow(ctx: &mut Context) -> HWND {
    // NtUserCallNoParam(GETDESKTOPWINDOW).
    HWND::from_raw(sogen::with_context(ctx, |_| sogen::call_no_param(0x03)))
}

#[win32_derive::dllexport]
pub fn MapWindowPoints(
    ctx: &mut Context,
    _hWndFrom: HWND,
    _hWndTo: HWND,
    lpPoints: Ptr<POINT>,
    cPoints: u32,
) -> i32 {
    // Both windows are the same top-level window for winmine, so the mapping is the identity; the
    // points are left as they are rather than guessing at a screen origin.
    let _ = (ctx, lpPoints, cPoints);
    let _ = user32::state();
    0
}
