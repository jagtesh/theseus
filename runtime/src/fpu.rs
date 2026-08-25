//! FPU registers.

use bitflags::bitflags;

bitflags! {
    pub struct Status: u16 {
        const C3 = 1 << 14;
        const C2 = 1 << 10;
        const C1 = 1 << 9;
        const C0 = 1 << 8;
    }
}

pub struct FPU {
    /// FPU ST0 through ST7 registers.
    pub st: [f64; 8],
    /// Index of top of FPU stack; 8 when stack empty.
    pub st_top: usize,
    /// The result of the last fcmp, used to generate status word.
    pub cmp: std::cmp::Ordering,
    /// Control word, as managed by fldcw/fnstcw. We only round-trip the value;
    /// precision/rounding control bits are not honored.
    pub control: u16,
}

impl Default for FPU {
    fn default() -> Self {
        Self {
            st: [0.; 8],
            st_top: 8,
            cmp: std::cmp::Ordering::Equal,
            control: 0x037f,
        }
    }
}

impl FPU {
    fn exception(_msg: &str) {
        // TODO: modify state bits etc.
        // At least ignoring these may allow programs to make some progress.
        // See note in https://github.com/joncampbell123/dosbox-x/issues/94 ,
        // "I've seen DOSBox SVN bail out on perfectly good demoscene programs because
        // of [not allowing underflow]."
        // Don't log because anatyda underflows thousands of times, eek.
        // log::warn!("{}", msg);
    }

    /// Get st(0), the current top of the FPU stack.
    pub fn st0(&mut self) -> &mut f64 {
        &mut self.st[self.st_top]
    }

    pub fn push(&mut self, val: f64) {
        if self.st_top == 0 {
            Self::exception("fpu stack overflow");
            return;
        }
        self.st_top -= 1;
        self.st[self.st_top] = val;
    }

    pub fn pop(&mut self) {
        if self.st_top == 8 {
            Self::exception("fpu stack underflow");
            return;
        }
        self.st_top += 1;
    }

    /// Index in self.st for a given ST0, ST1 etc reg.
    fn st_offset(&self, ofs: usize) -> usize {
        let new = self.st_top + ofs;
        if new >= 8 {
            Self::exception("fpu stack underflow");
            return 7;
        }
        new
    }

    pub fn swap(&mut self, o1: usize, o2: usize) {
        let o1 = self.st_offset(o1);
        let o2 = self.st_offset(o2);
        self.st.swap(o1, o2);
    }

    pub fn get(&self, ofs: usize) -> f64 {
        self.st[self.st_offset(ofs)]
    }

    pub fn set(&mut self, ofs: usize, val: f64) {
        self.st[self.st_offset(ofs)] = val;
    }

    pub fn status(&self) -> u16 {
        let status = match self.cmp {
            std::cmp::Ordering::Less => Status::C0,
            std::cmp::Ordering::Equal => Status::C3,
            std::cmp::Ordering::Greater => Status::empty(),
        };
        // Our status register impl doesn't include st_top so include it here.
        let mut status = status.bits();
        status |= (self.st_top as u16 & 0b111) << 11;
        status
    }

    pub fn round(&self, val: f64) -> f64 {
        // TODO: rounding modes?
        // This implements default rounding mode of round towards even.
        val.round_ties_even()
    }
}

/// x87 80-bit extended values, as seen by `fld`/`fstp tbyte ptr`. The register
/// file above is f64, so these convert on the way through and lose the low 11
/// mantissa bits; msvcrt's `long double` math is the main caller.
///
/// Layout: 64-bit mantissa with an *explicit* integer bit at bit 63, then a
/// 16-bit field holding the sign in bit 15 and a 15-bit exponent biased by
/// 16383. The explicit integer bit is what distinguishes this from f32/f64.
pub fn f80_to_f64(bytes: [u8; 10]) -> f64 {
    let mantissa = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let se = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
    let sign = (se >> 15) as u64;
    let exp = (se & 0x7fff) as i32;

    if exp == 0x7fff {
        let payload = mantissa & !(1 << 63);
        return if payload == 0 {
            if sign == 1 { f64::NEG_INFINITY } else { f64::INFINITY }
        } else {
            f64::NAN
        };
    }
    if exp == 0 && mantissa == 0 {
        return if sign == 1 { -0.0 } else { 0.0 };
    }

    let biased = exp - 16383 + 1023;
    if biased >= 0x7ff {
        return if sign == 1 { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    if biased <= 0 {
        // Representable as an f64 subnormal: rescale the mantissa to the fixed
        // 2^-1074 significand rather than flushing to zero.
        let shift = 12 - biased;
        let frac = if shift >= 64 { 0 } else { mantissa >> shift };
        return f64::from_bits(sign << 63 | frac);
    }

    let frac = (mantissa >> 11) & 0xf_ffff_ffff_ffff;
    f64::from_bits(sign << 63 | (biased as u64) << 52 | frac)
}

pub fn f64_to_f80(val: f64) -> [u8; 10] {
    let bits = val.to_bits();
    let sign = (bits >> 63) & 1;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & 0xf_ffff_ffff_ffff;

    let (mantissa, se) = if exp == 0x7ff {
        let mantissa = if frac == 0 { 1 << 63 } else { 1 << 63 | 1 << 62 };
        (mantissa, 0x7fff)
    } else if exp == 0 && frac == 0 {
        (0, 0)
    } else if exp == 0 {
        // f64 subnormal: normalize into the wider exponent range, which is
        // always representable there.
        let shift = frac.leading_zeros() - 11;
        let mantissa = (frac << (shift + 11)) | 1 << 63;
        (mantissa, (16383 - 1022 - shift as i32) as u16)
    } else {
        let mantissa = 1 << 63 | frac << 11;
        (mantissa, (exp - 1023 + 16383) as u16)
    };

    let mut out = [0u8; 10];
    out[0..8].copy_from_slice(&mantissa.to_le_bytes());
    out[8..10].copy_from_slice(&(se | (sign as u16) << 15).to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::{f64_to_f80, f80_to_f64};

    fn enc(mantissa: u64, se: u16) -> [u8; 10] {
        let mut out = [0u8; 10];
        out[0..8].copy_from_slice(&mantissa.to_le_bytes());
        out[8..10].copy_from_slice(&se.to_le_bytes());
        out
    }

    #[test]
    fn known_encodings() {
        // 1.0 and 2.0 differ only in the exponent; the explicit integer bit is set in both.
        assert_eq!(f80_to_f64(enc(0x8000_0000_0000_0000, 0x3fff)), 1.0);
        assert_eq!(f80_to_f64(enc(0x8000_0000_0000_0000, 0x4000)), 2.0);
        assert_eq!(f80_to_f64(enc(0x8000_0000_0000_0000, 0xbfff)), -1.0);
        assert_eq!(f80_to_f64(enc(0, 0)), 0.0);
        assert_eq!(f64_to_f80(1.0), enc(0x8000_0000_0000_0000, 0x3fff));
        assert_eq!(f64_to_f80(-1.0), enc(0x8000_0000_0000_0000, 0xbfff));
        assert_eq!(f64_to_f80(0.0), enc(0, 0));
    }

    #[test]
    fn round_trips() {
        for v in [
            1.0f64, -1.0, 0.5, -0.5, 2.0, 100.0, 1e300, -1e300, 1e-300,
            std::f64::consts::PI, f64::MIN_POSITIVE, f64::MAX,
        ] {
            assert_eq!(f80_to_f64(f64_to_f80(v)), v, "round trip {v}");
        }
    }

    #[test]
    fn specials() {
        assert!(f80_to_f64(f64_to_f80(f64::NAN)).is_nan());
        assert_eq!(f80_to_f64(f64_to_f80(f64::INFINITY)), f64::INFINITY);
        assert_eq!(f80_to_f64(f64_to_f80(f64::NEG_INFINITY)), f64::NEG_INFINITY);
        assert!(f80_to_f64(f64_to_f80(-0.0)).is_sign_negative());
    }

    #[test]
    fn subnormal_f64_round_trips() {
        let v = f64::from_bits(1);
        assert_eq!(f80_to_f64(f64_to_f80(v)), v);
        let v = f64::from_bits(0xf_ffff_ffff_ffff);
        assert_eq!(f80_to_f64(f64_to_f80(v)), v);
    }
}
