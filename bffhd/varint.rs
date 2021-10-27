use std::default::Default;
use std::ops::{Deref};

pub struct VarUInt<const N: usize> {
    offset: usize,
    bytes: [u8; N],
}

impl<const N: usize> VarUInt<N> {
    #[inline(always)]
    const fn new(bytes: [u8; N], offset: usize) -> Self {
        Self { bytes, offset }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[self.offset..]
    }

    #[inline(always)]
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.bytes[..]
    }

    #[inline(always)]
    pub const fn into_bytes(self) -> [u8; N] {
        self.bytes
    }

}

impl<const N: usize> Default for VarUInt<N> {
    fn default() -> Self {
        Self::new([0u8; N], N)
    }
}

impl<const N: usize> Deref for VarUInt<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

macro_rules! convert_from {
    ( $x:ty ) => {
        fn from(inp: $x) -> Self {
            let mut num = inp;
            let mut this = Self::default();
            let bytes = this.as_mut_bytes();

            let mut more = 0u8;
            let mut idx: usize = bytes.len()-1;

            while num > 0x7f {
                bytes[idx] = ((num & 0x7f) as u8 | more);
                num >>= 7;
                more = 0x80;
                idx -= 1;
            }
            bytes[idx] = (num as u8) | more;

            this.offset = idx;
            this
        }
    }
}

macro_rules! convert_into {
    ( $x:ty  ) => {
        fn into(self) -> $x {
            let mut out = 0;

            //  [0,1,2,3,4,5,6,7,8,9]
            // ^ 0
            //             ^offset = 5
            //                       ^ len = 10
            //             ^---------^ # of valid bytes = (len - offset)
            // for i in offset..len ⇒ all valid idx
            let bytes = self.as_bytes();
            let len = bytes.len();
            let mut shift = 0;

            for neg in 1..=len {
                let idx = len-neg;
                let val = (bytes[idx] & 0x7f) as $x;
                let shifted = val << shift;
                out |= shifted;
                shift += 7;
            }

            out
        }
    }
}

macro_rules! impl_convert_from_to {
    ( $num:ty, $req:literal, $nt:ident ) => {
        pub type $nt = VarUInt<$req>;
        impl From<$num> for VarUInt<$req> {
            convert_from! { $num }
        }
        impl Into<$num> for VarUInt<$req> {
            convert_into! { $num }
        }
    }
}

impl_convert_from_to!(u8, 2, VarU8);
impl_convert_from_to!(u16, 3, VarU16);
impl_convert_from_to!(u32, 5, VarU32);
impl_convert_from_to!(u64, 10, VarU64);
impl_convert_from_to!(u128, 19, VarU128);

#[allow(dead_code)]
#[cfg(target_pointer_width = "64")]
type VarUsize = VarU64;
#[cfg(target_pointer_width = "32")]
type VarUsize = VarU32;
#[cfg(target_pointer_width = "16")]
type VarUsize = VarU16;

impl<T, const N: usize> From<&T> for VarUInt<N>
    where T: Copy,
          VarUInt<N>: From<T>
{
    fn from(t: &T) -> Self {
        (*t).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varuint() {
        let inp = u64::MAX;
        let vi: VarU64 = inp.into();
        println!("Encoded {} into {:?}", inp, vi.as_bytes());
        let outp: u64 = vi.into();
        assert_eq!(inp, outp);

        let inp = 0x80;
        let vi: VarUInt<10> = inp.into();
        println!("Encoded {} into {:?}", inp, vi.as_bytes());
        let outp: u64 = vi.into();
        assert_eq!(inp, outp);
    }

    #[test]
    fn minimal() {
        let a = 5u8;
        assert_eq!(VarU8::from(a).as_bytes(), &[a]);
        let a = 200u8;
        assert_eq!(VarU8::from(a).as_bytes(), &[129, 72]);

        let inp = 128;
        let vi: VarU32 = inp.into();
        let expected: &[u8] = &[129, 0];
        assert_eq!(vi.as_bytes(), expected)
    }
}