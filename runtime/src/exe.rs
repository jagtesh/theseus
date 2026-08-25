use crate::{Cont, Context, Memory, Regs, mapping::Mappings};

pub struct EXEData {
    pub image_base: u32,
    /// Highest address the image's own mappings reach. Linked modules load at
    /// their own preferred bases, which are far above the main image, so the
    /// address space has to be sized from this rather than from a fixed guess.
    pub image_end: u32,
    pub resources: std::ops::Range<u32>,
    pub blocks: &'static [(u32, fn(&mut Context) -> Cont)],
    pub init: fn(&mut Regs, &mut Memory, &mut Mappings),
    pub entry_point: Cont,
}
