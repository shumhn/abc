use crate::error::Error;
use core::mem::MaybeUninit;

pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    for byte in dest.iter_mut() {
        byte.write(0);
    }
    Ok(())
}

pub fn inner(dest: &mut [u8]) -> Result<(), Error> {
    for byte in dest.iter_mut() {
        *byte = 0;
    }
    Ok(())
}

pub fn inner_u32() -> Result<u32, Error> {
    Ok(0)
}

pub fn inner_u64() -> Result<u64, Error> {
    Ok(0)
}
