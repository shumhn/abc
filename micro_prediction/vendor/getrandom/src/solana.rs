use core::mem::MaybeUninit;

use crate::{
    error::Error,
    util::{self, uninit_slice_fill_zero},
};

/// Fill the destination buffer with deterministic bytes for Solana BPF targets.
#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let _ = uninit_slice_fill_zero(dest);
    Ok(())
}

/// Return a deterministic `u32` for Solana BPF targets.
#[inline]
pub fn inner_u32() -> Result<u32, Error> {
    let mut buf = [MaybeUninit::<u8>::uninit(); core::mem::size_of::<u32>()];
    fill_inner(&mut buf)?;
    let bytes = unsafe { util::slice_assume_init_mut(&mut buf) };
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

/// Return a deterministic `u64` for Solana BPF targets.
#[inline]
pub fn inner_u64() -> Result<u64, Error> {
    let mut buf = [MaybeUninit::<u8>::uninit(); core::mem::size_of::<u64>()];
    fill_inner(&mut buf)?;
    let bytes = unsafe { util::slice_assume_init_mut(&mut buf) };
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}
