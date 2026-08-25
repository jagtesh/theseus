use runtime::Context;

use crate::{
    HANDLE, POINT, Ptr,
    gdi32::COLORREF,
    sogen,
};

pub type HDC = HANDLE;

#[win32_derive::dllexport]
pub fn CreateCompatibleDC(ctx: &mut Context, hdc: HDC) -> HDC {
    HDC::from_raw(sogen::with_context(ctx, |_| {
        sogen::create_compatible_dc(hdc.to_raw())
    }))
}

#[win32_derive::dllexport]
pub fn DeleteDC(ctx: &mut Context, hdc: HDC) -> bool {
    sogen::with_context(ctx, |_| sogen::delete_object(hdc.to_raw())) != 0
}

#[win32_derive::dllexport]
pub fn GetLayout(_ctx: &mut Context, _hdc: HDC) -> u32 {
    0 // LTR; gdi32 answers this client-side and issues no system call
}

#[win32_derive::dllexport]
pub fn SetLayout(_ctx: &mut Context, _hdc: HDC, _l: u32 /* DC_LAYOUT */) -> u32 {
    0
}

#[win32_derive::dllexport]
pub fn SetROP2(ctx: &mut Context, hdc: HDC, rop2: u32) -> i32 {
    // SetROP2 never reaches the kernel: gdi32 stores jROP2 in the DC's attribute block.
    sogen::with_context(ctx, |_| sogen::set_rop2(hdc.to_raw(), rop2)) as i32
}

#[win32_derive::dllexport]
pub fn LineTo(ctx: &mut Context, hdc: HDC, x: i32, y: i32) -> bool {
    sogen::with_context(ctx, |_| sogen::line_to(hdc.to_raw(), x, y)) != 0
}

#[win32_derive::dllexport]
pub fn MoveToEx(ctx: &mut Context, hdc: HDC, x: i32, y: i32, lppt: Ptr<POINT>) -> bool {
    // NtGdiMoveTo is in the service table with no handler because gdi32 writes ptlCurrent into the
    // DC's attribute block and issues no call at all.
    let mut previous = [0i32; 2];
    let moved = sogen::with_context(ctx, |_| {
        sogen::move_to(hdc.to_raw(), x, y, previous.as_mut_ptr())
    });

    if lppt.addr != 0 {
        lppt.write(
            &mut ctx.memory,
            POINT {
                x: previous[0],
                y: previous[1],
            },
        )
        .unwrap();
    }

    moved
}

#[win32_derive::dllexport]
pub fn SetPixel(ctx: &mut Context, hdc: HDC, x: i32, y: i32, color: COLORREF) -> COLORREF {
    let _ = (ctx, hdc, x, y);
    color
}
