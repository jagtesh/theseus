//! Sogen's win32k, linked in as a native library.
//!
//! This replaces the hand-written window manager and graphics engine with Windows 2000's own, as
//! reimplemented in Sogen's C++ and compiled for the host. Nothing here emulates a CPU: the
//! application is this crate's translated Rust, the kernel is C++, and the seam is a C ABI.
//!
//! Two things make the join work:
//!
//! * `Memory` is Sogen's mapping. `runtime::Memory` is a flat byte array addressed from zero, and so
//!   is the guest address space win32k reads through `memory_interface`. Adopting Sogen's mapping
//!   makes them the same bytes, so a pointer this crate writes is the pointer win32k dereferences.
//! * A window procedure is a function pointer. win32k calls up into the client constantly --
//!   WM_NCCREATE, WM_NCCALCSIZE, every dispatch -- and for a native client that is a plain call, with
//!   no stack marshalling and no CPU to resume.

#![allow(non_snake_case)]

use std::cell::Cell;
use std::ffi::{CString, c_void};
use std::os::raw::c_char;

use runtime::Context;

#[repr(C)]
pub struct Session {
    _private: [u8; 0],
}

#[repr(C)]
struct Config {
    struct_size: u32,
    root: *const c_char,
    argv0: *const c_char,
    memory_base: *mut c_void,
    memory_span: u64,
    image_base: u32,
    image_size: u32,
    image_name: *const c_char,
    client_arena_end: u32,
    verbose: i32,
}

type WindowProcedure =
    unsafe extern "C" fn(user: *mut c_void, hwnd: u32, message: u32, wparam: u32, lparam: u32) -> u64;

#[link(name = "sogen-win32k")]
unsafe extern "C" {
    fn sogen_win32k_create(config: *const Config) -> *mut Session;
    fn sogen_win32k_last_error() -> *const c_char;
    fn sogen_win32k_memory_base(session: *mut Session) -> *mut u8;
    fn sogen_win32k_memory_span(session: *mut Session) -> u64;
    fn sogen_win32k_set_window_procedure(
        session: *mut Session,
        procedure: Option<WindowProcedure>,
        user: *mut c_void,
    );
    fn sogen_win32k_pump(session: *mut Session);
    fn sogen_win32k_scratch(session: *mut Session, size: u32, alignment: u32) -> u32;
    fn sogen_win32k_scratch_mark(session: *mut Session) -> u32;
    fn sogen_win32k_scratch_release(session: *mut Session, mark: u32);

    fn sogen_NtUserGetThreadState(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtGdiInit(session: *mut Session) -> u64;

    fn sogen_NtUserRegisterClassExWOW(
        session: *mut Session,
        a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32,
    ) -> u64;
    fn sogen_NtUserCreateWindowEx(
        session: *mut Session,
        a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32,
        a9: u32, a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32, a16: u32,
    ) -> u64;
    fn sogen_NtUserShowWindow(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserMoveWindow(session: *mut Session, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32) -> u64;
    fn sogen_NtUserInvalidateRect(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtUserValidateRect(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserBeginPaint(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserEndPaint(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserGetDC(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtUserCallOneParam(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserCallNoParam(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtUserCallHwndLock(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserMessageCall(
        session: *mut Session,
        a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32,
    ) -> u64;
    fn sogen_NtUserGetMessage(session: *mut Session, a0: u32, a1: u32, a2: u32, a3: u32) -> u64;
    fn sogen_NtUserPeekMessage(session: *mut Session, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32) -> u64;
    fn sogen_NtUserTranslateMessage(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtUserDispatchMessage(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtUserDestroyWindow(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtUserSetMenu(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtUserCheckMenuItem(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtUserSystemParametersInfo(session: *mut Session, a0: u32, a1: u32, a2: u32, a3: u32) -> u64;

    fn sogen_NtGdiCreateCompatibleDC(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtGdiCreateCompatibleBitmap(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtGdiCreateBitmap(session: *mut Session, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32) -> u64;
    fn sogen_NtGdiCreateSolidBrush(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtGdiCreatePen(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtGdiSelectBitmap(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtGdiSelectPen(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtGdiGetStockObject(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtGdiGetDeviceCaps(session: *mut Session, a0: u32, a1: u32) -> u64;
    fn sogen_NtGdiDeleteObjectApp(session: *mut Session, a0: u32) -> u64;
    fn sogen_NtGdiLineTo(session: *mut Session, a0: u32, a1: u32, a2: u32) -> u64;
    fn sogen_NtGdiBitBlt(
        session: *mut Session,
        a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32, a9: u32, a10: u32,
    ) -> u64;
    fn sogen_NtGdiSetDIBitsToDeviceInternal(
        session: *mut Session,
        a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32, a7: u32, a8: u32,
        a9: u32, a10: u32, a11: u32, a12: u32, a13: u32, a14: u32, a15: u32,
    ) -> u64;

    fn sogen_gdi_object_type(session: *mut Session, handle: u32) -> u32;
    fn sogen_gdi_select_brush(session: *mut Session, dc: u32, brush: u32) -> u32;
    fn sogen_gdi_select_font(session: *mut Session, dc: u32, font: u32) -> u32;
    fn sogen_gdi_set_rop2(session: *mut Session, dc: u32, rop: u32) -> u32;
    fn sogen_gdi_move_to(session: *mut Session, dc: u32, x: i32, y: i32, previous: *mut i32) -> i32;
}

/// winmine's image, as `tc` placed it. The kernel maps a module here so the process it attributes
/// windows and classes to is the real one.
pub const IMAGE_BASE: u32 = 0x0100_0000;
pub const IMAGE_SIZE: u32 = 0x0001_A000;
/// Everything below this belongs to this crate's own allocator; the kernel places the PEB, the TEBs
/// and the desktop heap above it.
pub const CLIENT_ARENA_END: u32 = 0x0200_0000;

struct Global(Cell<*mut Session>);
unsafe impl Sync for Global {}
static SESSION: Global = Global(Cell::new(std::ptr::null_mut()));

thread_local! {
    /// The context the current ABI call was made from, so a window-procedure callback arriving in
    /// the middle of one can run translated code. win32k calls the client synchronously and on this
    /// thread, so the pointer is live for exactly the duration of the call that set it.
    static CONTEXT: Cell<*mut Context> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn session() -> *mut Session {
    SESSION.0.get()
}

pub fn last_error() -> String {
    unsafe {
        std::ffi::CStr::from_ptr(sogen_win32k_last_error())
            .to_string_lossy()
            .into_owned()
    }
}

/// Runs `body` with `ctx` reachable from the window-procedure callback. win32k calls the client
/// synchronously and on this thread, so the pointer is live for exactly the duration of the call.
pub fn with_context<T>(ctx: &mut Context, body: impl FnOnce(&mut Context) -> T) -> T {
    let pointer = ctx as *mut Context;
    let previous = CONTEXT.with(|slot| slot.replace(pointer));
    let result = body(unsafe { &mut *pointer });
    CONTEXT.with(|slot| slot.set(previous));
    result
}

unsafe extern "C" fn window_procedure(
    _user: *mut c_void,
    hwnd: u32,
    message: u32,
    wparam: u32,
    lparam: u32,
) -> u64 {
    let ctx = CONTEXT.with(|slot| slot.get());
    if ctx.is_null() {
        log::warn!("window procedure callback with no context: msg {message:#x}");
        return 0;
    }

    let ctx = unsafe { &mut *ctx };
    crate::user32::dispatch_to_wndproc(ctx, hwnd, message, wparam, lparam)
}

/// Starts win32k and hands back its guest memory. Called before anything else in the process.
pub fn boot() -> &'static mut [u8] {
    let root = std::env::var("SOGEN_STANDALONE_ROOT").unwrap_or_else(|_| env!("SOGEN_ROOT").to_string());
    let root = CString::new(root).unwrap();
    let name = CString::new("winmine.exe").unwrap();

    let config = Config {
        struct_size: std::mem::size_of::<Config>() as u32,
        root: root.as_ptr(),
        argv0: std::ptr::null(),
        memory_base: std::ptr::null_mut(),
        memory_span: 0,
        image_base: IMAGE_BASE,
        image_size: IMAGE_SIZE,
        image_name: name.as_ptr(),
        client_arena_end: CLIENT_ARENA_END,
        verbose: if std::env::var("SOGEN_VERBOSE").is_ok() { 1 } else { 0 },
    };

    let session = unsafe { sogen_win32k_create(&config) };
    assert!(!session.is_null(), "win32k failed to start: {}", last_error());
    SESSION.0.set(session);

    unsafe { sogen_win32k_set_window_procedure(session, Some(window_procedure), std::ptr::null_mut()) };

    // The connect call user32 makes on first use; it is what publishes the shared sections the
    // client half reads (the GDI handle table among them).
    unsafe { sogen_NtUserGetThreadState(session, 0x11) };
    unsafe { sogen_NtGdiInit(session) };

    let base = unsafe { sogen_win32k_memory_base(session) };
    let span = unsafe { sogen_win32k_memory_span(session) };
    unsafe { std::slice::from_raw_parts_mut(base, span as usize) }
}

pub fn pump() {
    unsafe { sogen_win32k_pump(session()) };
}

pub fn scratch(size: u32, alignment: u32) -> u32 {
    unsafe { sogen_win32k_scratch(session(), size, alignment) }
}

pub fn scratch_mark() -> u32 {
    unsafe { sogen_win32k_scratch_mark(session()) }
}

pub fn scratch_release(mark: u32) {
    unsafe { sogen_win32k_scratch_release(session(), mark) }
}

macro_rules! forward {
    ($name:ident, $abi:ident $(, $arg:ident : $ty:ty)*) => {
        pub fn $name($($arg: $ty),*) -> u32 {
            unsafe { $abi(session() $(, $arg as u32)*) as u32 }
        }
    };
}

forward!(register_class, sogen_NtUserRegisterClassExWOW, a0: u32, a1: u32, a2: u32, a3: u32, a4: u32, a5: u32, a6: u32);
forward!(show_window, sogen_NtUserShowWindow, hwnd: u32, cmd: u32);
forward!(move_window, sogen_NtUserMoveWindow, hwnd: u32, x: i32, y: i32, w: i32, h: i32, repaint: u32);
forward!(invalidate_rect, sogen_NtUserInvalidateRect, hwnd: u32, rect: u32, erase: u32);
forward!(validate_rect, sogen_NtUserValidateRect, hwnd: u32, rect: u32);
forward!(begin_paint, sogen_NtUserBeginPaint, hwnd: u32, paint: u32);
forward!(end_paint, sogen_NtUserEndPaint, hwnd: u32, paint: u32);
forward!(get_dc, sogen_NtUserGetDC, hwnd: u32);
forward!(call_one_param, sogen_NtUserCallOneParam, param: u32, routine: u32);
forward!(call_no_param, sogen_NtUserCallNoParam, routine: u32);
forward!(call_hwnd_lock, sogen_NtUserCallHwndLock, hwnd: u32, code: u32);
forward!(message_call, sogen_NtUserMessageCall, hwnd: u32, msg: u32, w: u32, l: u32, result: u32, kind: u32, ansi: u32);
forward!(get_message, sogen_NtUserGetMessage, msg: u32, hwnd: u32, lo: u32, hi: u32);
forward!(peek_message, sogen_NtUserPeekMessage, msg: u32, hwnd: u32, lo: u32, hi: u32, remove: u32);
forward!(translate_message, sogen_NtUserTranslateMessage, msg: u32, flags: u32);
forward!(dispatch_message, sogen_NtUserDispatchMessage, msg: u32);
forward!(destroy_window, sogen_NtUserDestroyWindow, hwnd: u32);
forward!(set_menu, sogen_NtUserSetMenu, hwnd: u32, menu: u32, redraw: u32);
forward!(check_menu_item, sogen_NtUserCheckMenuItem, menu: u32, item: u32, check: u32);
forward!(system_parameters_info, sogen_NtUserSystemParametersInfo, action: u32, param: u32, pv: u32, ini: u32);

forward!(create_compatible_dc, sogen_NtGdiCreateCompatibleDC, dc: u32);
forward!(create_compatible_bitmap, sogen_NtGdiCreateCompatibleBitmap, dc: u32, w: u32, h: u32);
forward!(create_bitmap, sogen_NtGdiCreateBitmap, w: u32, h: u32, planes: u32, bpp: u32, bits: u32);
forward!(create_solid_brush, sogen_NtGdiCreateSolidBrush, color: u32, unused: u32);
forward!(create_pen, sogen_NtGdiCreatePen, style: u32, width: u32, color: u32);
forward!(select_bitmap, sogen_NtGdiSelectBitmap, dc: u32, bitmap: u32);
forward!(select_pen, sogen_NtGdiSelectPen, dc: u32, pen: u32);
forward!(get_stock_object, sogen_NtGdiGetStockObject, index: u32);
forward!(get_device_caps, sogen_NtGdiGetDeviceCaps, dc: u32, index: u32);
forward!(delete_object, sogen_NtGdiDeleteObjectApp, handle: u32);
forward!(line_to, sogen_NtGdiLineTo, dc: u32, x: i32, y: i32);

forward!(object_type, sogen_gdi_object_type, handle: u32);
forward!(select_brush, sogen_gdi_select_brush, dc: u32, brush: u32);
forward!(select_font, sogen_gdi_select_font, dc: u32, font: u32);
forward!(set_rop2, sogen_gdi_set_rop2, dc: u32, rop: u32);

pub fn move_to(dc: u32, x: i32, y: i32, previous: *mut i32) -> bool {
    unsafe { sogen_gdi_move_to(session(), dc, x, y, previous) != 0 }
}

#[allow(clippy::too_many_arguments)]
pub fn bit_blt(
    dst: u32, x: i32, y: i32, w: i32, h: i32, src: u32, sx: i32, sy: i32, rop: u32,
) -> bool {
    unsafe {
        sogen_NtGdiBitBlt(
            session(), dst, x as u32, y as u32, w as u32, h as u32, src, sx as u32, sy as u32, rop, 0, 0,
        ) != 0
    }
}

#[allow(clippy::too_many_arguments)]
pub fn set_dibits_to_device(
    dc: u32, x: i32, y: i32, w: u32, h: u32, sx: i32, sy: i32, start_scan: u32, scan_lines: u32,
    bits: u32, info: u32, color_use: u32, max_bits: u32,
) -> u32 {
    unsafe {
        sogen_NtGdiSetDIBitsToDeviceInternal(
            session(), dc, x as u32, y as u32, w, h, sx as u32, sy as u32, start_scan, scan_lines,
            bits, info, color_use, max_bits, 0, 1, 0,
        ) as u32
    }
}

pub fn create_window(
    ex_style: u32, class_name: u32, window_name: u32, style: u32,
    x: i32, y: i32, width: i32, height: i32,
    parent: u32, menu: u32, instance: u32, param: u32,
) -> u32 {
    // Windows 2000's NtUserCreateWindowEx has no plstrClsVersion parameter, so every argument after
    // the class name sits one slot early; the kernel's own handler undoes the shift.
    unsafe {
        sogen_NtUserCreateWindowEx(
            session(), ex_style, class_name, window_name, style,
            x as u32, y as u32, width as u32, height as u32,
            parent, menu, instance, param, 0, 0, 0, 0, 0,
        ) as u32
    }
}

// ---- the guest structures win32k takes its arguments in ----

use runtime::Memory;

pub fn wstr_units(memory: &Memory, addr: u32) -> u32 {
    let mut units = 0u32;
    while memory.read::<u16>(addr + units * 2) != 0 {
        units += 1;
    }
    units
}

/// UNICODE_STRING: Length, MaximumLength, Buffer.
pub fn unicode_string(memory: &mut Memory, buffer: u32, units: u32) -> u32 {
    let addr = scratch(8, 4);
    memory.write::<u32>(addr, (units * 2) | ((units * 2 + 2) << 16));
    memory.write::<u32>(addr + 4, buffer);
    addr
}

/// LARGE_STRING: Length, MaximumLength:31 | bAnsi:1, Buffer.
pub fn large_string(memory: &mut Memory, buffer: u32, units: u32) -> u32 {
    let addr = scratch(12, 4);
    memory.write::<u32>(addr, units * 2);
    memory.write::<u32>(addr + 4, units * 2 + 2);
    memory.write::<u32>(addr + 8, buffer);
    addr
}

pub fn write_wstr(memory: &mut Memory, text: &str) -> (u32, u32) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let addr = scratch((units.len() as u32 + 1) * 2, 2);
    for (index, unit) in units.iter().enumerate() {
        memory.write::<u16>(addr + index as u32 * 2, *unit);
    }
    memory.write::<u16>(addr + units.len() as u32 * 2, 0);
    (addr, units.len() as u32)
}

/// NtUserCallOneParam(dc, RELEASEDC).
pub const ONEPARAM_RELEASEDC: u32 = 57;
/// NtUserCallHwndLock(hwnd, UPDATEWINDOW).
pub const HWNDLOCK_UPDATEWINDOW: u32 = 0x36;
/// The fnid the client thunk passes for DefWindowProc.
pub const FNID_DEFWINDOW: u32 = 0x29E;

pub fn def_window_proc(memory: &mut Memory, hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
    let mark = scratch_mark();
    let result = scratch(4, 4);
    memory.write::<u32>(result, 0);
    message_call(hwnd, msg, wparam, lparam, result, FNID_DEFWINDOW, 0);
    let value = memory.read::<u32>(result);
    scratch_release(mark);
    value
}
