use candid::{CandidType, Decode, Encode, Nat};
use ic_stable_structures::{storable::Bound, Storable};
use num_traits::ToPrimitive;

use std::borrow::Cow;

use serde::Deserialize;

type Amount = u128;
type Time = u64;

pub const _ONE_BASIS_POINT: u64 = 1000;

pub const _ONE_PERCENT: u64 = 100_000;

pub const _BASE_UNITS: Amount = 1_000_000_000;

const YEAR: Time = 31_536_000_000_000_000;

const MONTH: Time = 2_628_000_000_000_000;

#[derive(Copy, Clone, Deserialize, CandidType)]
pub enum StakeSpan {
    Instant,
    Month2,
    Month6,
    Year,
}

#[derive(Deserialize, CandidType, Copy, Clone)]
pub struct StakeDetails {
    pub stake_span: StakeSpan,
    pub amount: Amount,
    pub expiry_time: Time,
    pub pre_earnings: Amount,
}

impl Storable for StakeDetails {
    const BOUND: Bound = Bound::Bounded {
        max_size: 50,
        is_fixed_size: true,
    };
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

#[derive(CandidType, Deserialize, Default, Clone)]
pub struct VaultStakingDetails {
    pub span0_details: StakeDurationDetails,
    pub span2_details: StakeDurationDetails,
    pub span6_details: StakeDurationDetails,
    pub span12_details: StakeDurationDetails,
}

impl VaultStakingDetails {
    /// Create Stake function
    ///
    ///
    /// Params
    ///  - Amount :The amount of asset being put staked or deposited
    ///  - Current Lifetime Earnings :The total amount since first epoch of asset  received as fees to leverage provider from traders trading with leverage
    ///  - Stake Span :The specific staking duration
    ///
    /// Returns
    ///  - StakeDetails :The details of the newly created stake
    pub fn _create_stake(
        &mut self,
        amount: Amount,
        current_lifetime_earnings: Amount,
        stake_span: StakeSpan,
    ) -> StakeDetails {
        let (span_lifetime_earnings_per_token, span_init_total_locked, expiry_time) =
            self._update_asset_staking_details(amount, current_lifetime_earnings, stake_span, true);
        //
        let pre_earnings = if span_init_total_locked == Nat::from(0 as u128) {
            Nat::from(0 as u128)
        } else {
            (Nat::from(amount) * span_lifetime_earnings_per_token) / base_units()
        };

        let stake_details = StakeDetails {
            stake_span,
            amount,
            pre_earnings: pre_earnings.0.to_u128().unwrap(),
            expiry_time,
        };

        return stake_details;
    }

    /// Close Stake Function
    ///
    /// Params
    ///  - Reference Stake :The stake details of the reference stake to close
    ///  - Current Lifetime Earnings :The total amount since first epoch of asset  received as fees to leverage provider from traders trading with leverage
    ///
    /// Returns
    ///  - Earnings :The amount earned by the particular stake for the entire staking duration
    pub fn _close_stake(
        &mut self,
        reference_stake: StakeDetails,
        current_lifetime_earnings: Amount,
    ) -> Amount {
        let (lifetime_earnings_per_token, _, _) = self._update_asset_staking_details(
            reference_stake.amount,
            current_lifetime_earnings,
            reference_stake.stake_span,
            false,
        );

        let amount_earned =
            (Nat::from(reference_stake.amount) * lifetime_earnings_per_token) / base_units();

        let user_earnings = amount_earned.0.to_u128().unwrap() - reference_stake.pre_earnings;

        return user_earnings;
    }

    /// Update Asset Staking Details Function
    ///
    /// Params
    /// - Amount :The amount of asset being staked or unstaked
    /// - Current Lifetime Earnings :The total amount since first epoch of asset  received as fees to leverage provider from traders trading with leverage
    /// - Specific Span :The specific stake duration
    /// - Lock :true if staking and false if unstaking
    ///
    /// Returns
    /// - Lifetime Earnings Per Token :The amount earned per token staked in that particular stake duration since first epoch
    /// - Initial Locked Amount :The Amount of asset (token) locked in that particular stake span
    /// - Expiry Time :The expiry time in the future in which a stake placed now can be removed ,basically the current timestamp + the stake span duration
    pub fn _update_asset_staking_details(
        &mut self,
        amount: Amount,
        current_lifetime_earnings: Amount,
        specific_span: StakeSpan,
        lock: bool,
    ) -> (Nat, Amount, Time) {
        let span_lifetime_earnings_per_token;
        let span_init_total_locked;
        let expiry_time;

        match specific_span {
            StakeSpan::Instant => {
                //    let mut specific_span_details = self.span0_details;
                span_init_total_locked = self.span0_details.total_locked;
                span_lifetime_earnings_per_token =
                    self.span0_details
                        .update(amount, None, current_lifetime_earnings, lock);
                expiry_time = ic_cdk::api::time()
            }
            StakeSpan::Month2 => {
                span_init_total_locked = self.span2_details.total_locked;
                span_lifetime_earnings_per_token =
                    self.span2_details
                        .update(amount, Some(2), current_lifetime_earnings, lock);
                expiry_time = ic_cdk::api::time() + (2 * MONTH);
            }
            StakeSpan::Month6 => {
                span_init_total_locked = self.span6_details.total_locked;
                span_lifetime_earnings_per_token =
                    self.span6_details
                        .update(amount, Some(6), current_lifetime_earnings, lock);
                expiry_time = ic_cdk::api::time() + (6 * MONTH)
            }
            StakeSpan::Year => {
                span_init_total_locked = self.span12_details.total_locked;
                span_lifetime_earnings_per_token =
                    self.span12_details
                        .update(amount, Some(12), current_lifetime_earnings, lock);
                expiry_time = ic_cdk::api::time() + YEAR
            }
        }

        return (
            span_lifetime_earnings_per_token,
            span_init_total_locked,
            expiry_time,
        );
    }
}

impl Storable for VaultStakingDetails {
    const BOUND: Bound = Bound::Unbounded;
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

#[derive(Clone, Deserialize, CandidType, Default)]
pub struct StakeDurationDetails {
    ///The  last amount recorded as the total amount since first epoch of asset  received as fees to leverage provider from traders trading with leverage  
    pub prev_all_time_earnings: Amount,
    ///Life Time Earnings
    ///
    /// The total Amount earned by a single token since
    pub lifetime_earnings_per_token: Nat,
    /// Total Locked
    ///
    /// The total Amount of liquidity locked in that particular span
    pub total_locked: Amount,
}

impl StakeDurationDetails {
    /// Update function
    /// Updates stake duration details
    /// Params
    ///  - Amount :The Amount being put in or removed from the particular stake duration
    ///  - Span Share :The share measure for the particular stake duration
    ///  - Current Lifetime Earnings :The total amount since first epoch of asset  received as fees to leverage provider from traders trading with leverage
    ///  - Lock :true if staking and false otherwise
    ///
    /// Returns
    ///
    ///  - Lifetime Earnings Per Token :The amount earned per token token staked since the beginning epoch
    pub fn update(
        &mut self,
        amount: Amount,
        span_share: Option<u128>,
        current_all_time_earnings: Amount,
        lock: bool,
    ) -> Nat {
        if current_all_time_earnings == self.prev_all_time_earnings {
            if lock {
                self.total_locked += amount
            } else {
                self.total_locked -= amount
            };

            return self.lifetime_earnings_per_token.clone();
        }
        let (percentage, share, total_share) = match span_share {
            Some(value) => (40 * _ONE_PERCENT, value, 20),
            None => (60 * _ONE_PERCENT, 1, 1),
        };

        let init_total_locked = if self.total_locked == 0 {
            1
        } else {
            self.total_locked
        };
        // new earnings
        let new_earnings = current_all_time_earnings - self.prev_all_time_earnings;

        let span_new_earnings = _percentage128(percentage, new_earnings);

        let span_new_earnings_per_token = (Nat::from(span_new_earnings * share as u128)
            * base_units())
            / Nat::from(total_share * init_total_locked);

        self.lifetime_earnings_per_token += span_new_earnings_per_token;

        if lock {
            self.total_locked += amount
        } else {
            self.total_locked -= amount
        };

        self.prev_all_time_earnings = current_all_time_earnings;

        return self.lifetime_earnings_per_token.clone();
    }
}

fn base_units() -> Nat {
    Nat::from((10 as u128).pow(25))
}

pub fn _percentage128(x: u64, value: Amount) -> Amount {
    return ((x as u128) * value) / (100 * _ONE_PERCENT as u128);
}

pub fn _percentage64(x: u64, value: u64) -> u64 {
    return (x * value) / (100 * _ONE_PERCENT);
}
