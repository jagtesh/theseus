use runtime::Context;

use crate::{gdi32::HDC, sogen};

pub type HGDIOBJ = u32;

#[win32_derive::dllexport]
pub fn DeleteObject(ctx: &mut Context, ho: HGDIOBJ) -> bool {
    sogen::with_context(ctx, |_| sogen::delete_object(ho)) != 0
}

#[win32_derive::dllexport]
pub fn GetDeviceCaps(ctx: &mut Context, hdc: HDC, index: u32) -> i32 {
    sogen::with_context(ctx, |_| sogen::get_device_caps(hdc.to_raw(), index)) as i32
}
