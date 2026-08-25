use runtime::Context;

use crate::{
    Ptr,
    gdi32::{COLORREF, HDC, HGDIOBJ, HPEN},
    sogen,
};

#[win32_derive::dllexport]
pub fn CreatePen(ctx: &mut Context, iStyle: u32 /* PEN_STYLE */, cWidth: i32, color: COLORREF) -> HPEN {
    HPEN::from_raw(sogen::with_context(ctx, |_| {
        sogen::create_pen(iStyle, cWidth as u32, color.as_win32())
    }))
}

#[win32_derive::dllexport]
pub fn CreateSolidBrush(ctx: &mut Context, color: COLORREF) -> HGDIOBJ {
    HGDIOBJ::from_raw(sogen::with_context(ctx, |_| {
        sogen::create_solid_brush(color.as_win32(), 0)
    }))
}

#[repr(C)]
#[derive(zerocopy::Immutable, zerocopy::IntoBytes)]
pub struct BITMAP {
    bmType: u32,
    bmWidth: u32,
    bmHeight: u32,
    bmWidthBytes: u32,
    bmPlanes: u16,
    bmBitsPixel: u16,
    bmBits: u32,
}

#[win32_derive::dllexport]
pub fn GetObjectA(ctx: &mut Context, _handle: HGDIOBJ, size: u32, lpOut: Ptr<BITMAP>) -> u32 {
    // winmine never reaches this; a zeroed answer is honest about that rather than inventing metrics.
    lpOut
        .write(
            &mut ctx.memory,
            BITMAP {
                bmType: 0,
                bmWidth: 0,
                bmHeight: 0,
                bmWidthBytes: 0,
                bmPlanes: 0,
                bmBitsPixel: 0,
                bmBits: 0,
            },
        )
        .unwrap();
    size
}

#[win32_derive::dllexport]
pub fn GetStockObject(ctx: &mut Context, i: u32) -> HGDIOBJ {
    HGDIOBJ::from_raw(sogen::with_context(ctx, |_| sogen::get_stock_object(i)))
}

/// The client-visible GDI object types, as the handle's own table cell carries them.
const GDI_TYPE_BITMAP: u32 = 0x05;
const GDI_TYPE_FONT: u32 = 0x0A;
const GDI_TYPE_BRUSH: u32 = 0x10;
const GDI_TYPE_PEN: u32 = 0x30;
const GDI_TYPE_EXTPEN: u32 = 0x50;

#[win32_derive::dllexport]
pub fn SelectObject(ctx: &mut Context, hdc: HDC, h: HGDIOBJ) -> HGDIOBJ {
    if h.is_null_or_invalid() {
        return HGDIOBJ::null();
    }

    // A bitmap and a pen go through their own services; a brush and a font are written into the DC's
    // attribute block, which is why NtGdiSelectBrush has no handler.
    let previous = sogen::with_context(ctx, |_| {
        let dc = hdc.to_raw();
        let object = h.to_raw();
        match sogen::object_type(object) {
            GDI_TYPE_BITMAP => sogen::select_bitmap(dc, object),
            GDI_TYPE_PEN | GDI_TYPE_EXTPEN => sogen::select_pen(dc, object),
            GDI_TYPE_BRUSH => sogen::select_brush(dc, object),
            GDI_TYPE_FONT => sogen::select_font(dc, object),
            kind => {
                log::warn!("SelectObject: unknown GDI object type {kind:#x} for {object:#x}");
                0
            }
        }
    });

    HGDIOBJ::from_raw(previous)
}
