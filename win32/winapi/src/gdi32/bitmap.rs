use runtime::Context;

use crate::{HANDLE, Ptr, gdi32::HDC, sogen};

pub type HBITMAP = HANDLE;

#[win32_derive::dllexport]
pub fn BitBlt(
    ctx: &mut Context,
    hdc: HDC,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    hdcSrc: HDC,
    x1: i32,
    y1: i32,
    rop: u32, /* ROP_CODE */
) -> bool {
    sogen::with_context(ctx, |_| {
        sogen::bit_blt(hdc.to_raw(), x, y, cx, cy, hdcSrc.to_raw(), x1, y1, rop)
    })
}

#[win32_derive::dllexport]
pub fn StretchBlt(
    ctx: &mut Context,
    hdcDest: HDC,
    xDest: i32,
    yDest: i32,
    wDest: i32,
    hDest: i32,
    hdcSrc: HDC,
    xSrc: i32,
    ySrc: i32,
    wSrc: i32,
    hSrc: i32,
    rop: u32, /* ROP_CODE */
) -> bool {
    // Sogen implements NtGdiStretchBlt, but winmine only ever blits 1:1, so the unscaled path is the
    // one wired up; a genuine stretch would be a different service rather than this one lying.
    assert_eq!(wDest, wSrc);
    assert_eq!(hDest, hSrc);
    BitBlt(ctx, hdcDest, xDest, yDest, wDest, hDest, hdcSrc, xSrc, ySrc, rop)
}

#[win32_derive::dllexport]
pub fn CreateCompatibleBitmap(ctx: &mut Context, hdc: HDC, cx: i32, cy: i32) -> HBITMAP {
    HBITMAP::from_raw(sogen::with_context(ctx, |_| {
        sogen::create_compatible_bitmap(hdc.to_raw(), cx as u32, cy as u32)
    }))
}

#[win32_derive::dllexport]
pub fn SetDIBitsToDevice(
    ctx: &mut Context,
    hdc: HDC,
    xDest: u32,
    yDest: u32,
    w: u32,
    h: u32,
    xSrc: u32,
    ySrc: u32,
    StartScan: u32,
    cLines: u32,
    lpvBits: Ptr<u8>,
    lpbmi: Ptr<u8>, /* BITMAPINFO */
    ColorUse: u32,  /* DIB_USAGE */
) -> u32 {
    // The bits and the BITMAPINFO are already at guest addresses the kernel can read -- they are in
    // winmine's own .rsrc -- so nothing is copied across the boundary.
    sogen::with_context(ctx, |_| {
        sogen::set_dibits_to_device(
            hdc.to_raw(),
            xDest as i32,
            yDest as i32,
            w,
            h,
            xSrc as i32,
            ySrc as i32,
            StartScan,
            cLines,
            lpvBits.addr,
            lpbmi.addr,
            ColorUse,
            0,
        )
    })
}
