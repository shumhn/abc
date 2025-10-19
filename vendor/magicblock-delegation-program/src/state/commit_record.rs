use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::{
    impl_to_bytes_with_discriminator_zero_copy, impl_try_from_bytes_with_discriminator_zero_copy,
};

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};

/// The Commit State Record
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct CommitRecord {
    /// The identity committing the state
    pub identity: Pubkey,

    /// The account for which the state is committed
    pub account: Pubkey,

    /// The external nonce of the commit. This is used to enforce sequential commits
    pub nonce: u64,

    /// The account committed lamports
    pub lamports: u64,
}

impl AccountWithDiscriminator for CommitRecord {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::CommitRecord
    }
}

impl CommitRecord {
    pub fn size_with_discriminator() -> usize {
        8 + size_of::<CommitRecord>()
    }
}

impl_to_bytes_with_discriminator_zero_copy!(CommitRecord);
impl_try_from_bytes_with_discriminator_zero_copy!(CommitRecord);
