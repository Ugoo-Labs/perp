use crate::core_lib::token::Asset;

use super::core_lib::staking::*;
use candid::{CandidType, Decode, Encode};

use serde::Deserialize;

use std::borrow::Cow;

use ic_stable_structures::{storable::Bound, Storable};

type Amount = u128;

#[derive(CandidType, Deserialize, Clone)]
pub struct VaultDetails {
    pub asset: Asset,
    pub virtaul_asset: Asset,
    pub tx_fee: Amount,
    pub min_amount: Amount,
    pub debt: Amount,
    pub free_liquidity: Amount,
    pub lifetime_fees: Amount,
    pub staking_details: VaultStakingDetails,
}

impl Default for VaultDetails {
    fn default() -> Self {
        VaultDetails {
            asset: Asset::default(),
            virtaul_asset: Asset::default(),
            tx_fee: 0,
            min_amount: 0,
            debt: 0,
            free_liquidity: 0,
            lifetime_fees: 0,
            staking_details: VaultStakingDetails::default(),
        }
    }
}

impl Storable for VaultDetails {
    const BOUND: Bound = Bound::Unbounded;
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}
