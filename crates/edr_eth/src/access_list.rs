// Part of this code was adapted from ethers-rs and is distributed under their
// license:
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-APACHE
// - https://github.com/gakonst/ethers-rs/blob/cba6f071aedafb766e82e4c2f469ed5e4638337d/LICENSE-MIT
// For the original context see: https://github.com/gakonst/ethers-rs/blob/3d9c3290d42b77c510e5b5d0b6f7a2f72913bfff/ethers-core/src/types/transaction/eip2930.rs

use alloy_rlp::{RlpDecodable, RlpDecodableWrapper, RlpEncodable, RlpEncodableWrapper};

use crate::{Address, B256, U256};

/// Access list
// NB: Need to use `RlpEncodableWrapper` else we get an extra [] in the output
// https://github.com/gakonst/ethers-rs/pull/353#discussion_r680683869
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, RlpDecodableWrapper, RlpEncodableWrapper)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccessList(pub Vec<AccessListItem>);

impl From<Vec<AccessListItem>> for AccessList {
    fn from(src: Vec<AccessListItem>) -> AccessList {
        AccessList(src)
    }
}

impl From<AccessList> for Vec<AccessListItem> {
    fn from(src: AccessList) -> Vec<AccessListItem> {
        src.0
    }
}
