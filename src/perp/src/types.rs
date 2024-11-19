use crate::corelib::calc_lib::{_calc_shares, _calc_shares_value, _percentage128};
use candid::{CandidType, Decode, Encode, Principal};
use ic_stable_structures::{storable::Bound, Storable};

use serde::Deserialize;
use std::borrow::Cow;
pub type Tick = u64;
pub type Time = u64;
pub type Amount = u128;

/// The enum defining the different asset classes.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq)]
pub enum AssetClass {
    /// The cryptocurrency asset class.
    Cryptocurrency,
    /// The fiat currency asset class.
    FiatCurrency,
}

impl Default for AssetClass {
    fn default() -> Self {
        AssetClass::Cryptocurrency
    }
}

/// Exchange rates are derived for pairs of assets captured in this struct.
#[derive(CandidType, Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Asset {
    /// The symbol/code of the asset.
    pub symbol: String,
    /// The asset class.
    pub class: AssetClass,
}

/// The type the user sends when requesting a rate.
///
/// For definitions of "base", "quote", etc, the reader is referred to
/// https://en.wikipedia.org/wiki/Currency_pair.
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct GetExchangeRateRequest {
    /// The base asset, i.e., the first asset in a currency pair. For example,
    /// ICP is the base asset in the currency pair ICP/USD.
    pub base_asset: Asset,
    /// The quote asset, i.e., the second asset in a currency pair. For example,
    /// USD is the quote asset in the currency pair ICP/USD.
    pub quote_asset: Asset,
    /// An optional parameter used to find a rate at a specific time.
    pub timestamp: Option<u64>,
}

/// Metadata information to give background on how the rate was determined.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq)]
pub struct ExchangeRateMetadata {
    /// The scaling factor for the exchange rate and the standard deviation.
    pub decimals: u32,
    /// The number of queried exchanges for the base asset.
    pub base_asset_num_queried_sources: usize,
    /// The number of rates successfully received from the queried sources for the base asset.
    pub base_asset_num_received_rates: usize,
    /// The number of queried exchanges for the quote asset.
    pub quote_asset_num_queried_sources: usize,
    /// The number of rates successfully received from the queried sources for the quote asset.
    pub quote_asset_num_received_rates: usize,
    /// The standard deviation of the received rates, scaled by the factor `10^decimals`.
    pub standard_deviation: u64,
    /// The timestamp of the beginning of the day for which the forex rates were retrieved, if any.
    pub forex_timestamp: Option<u64>,
}

/// When a rate is determined, this struct is used to present the information
/// to the user.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq)]
pub struct ExchangeRate {
    /// The base asset.
    pub base_asset: Asset,
    /// The quote asset.
    pub quote_asset: Asset,
    /// The timestamp associated with the returned rate.
    pub timestamp: u64,
    /// The median rate from the received rates, scaled by the factor `10^decimals` in the metadata.
    pub rate: u64,
    /// Metadata providing additional information about the exchange rate calculation.
    pub metadata: ExchangeRateMetadata,
}

/// Returned to the user when something goes wrong retrieving the exchange rate.
#[derive(CandidType, Clone, Debug, Deserialize)]
pub enum ExchangeRateError {
    /// Returned when the canister receives a call from the anonymous principal.
    AnonymousPrincipalNotAllowed,
    /// Returned when the canister is in process of retrieving a rate from an exchange.
    Pending,
    /// Returned when the base asset rates are not found from the exchanges HTTP outcalls.
    CryptoBaseAssetNotFound,
    /// Returned when the quote asset rates are not found from the exchanges HTTP outcalls.
    CryptoQuoteAssetNotFound,
    /// Returned when the stablecoin rates are not found from the exchanges HTTP outcalls needed for computing a crypto/fiat pair.
    StablecoinRateNotFound,
    /// Returned when there are not enough stablecoin rates to determine the forex/USDT rate.
    StablecoinRateTooFewRates,
    /// Returned when the stablecoin rate is zero.
    StablecoinRateZeroRate,
    /// Returned when a rate for the provided forex asset could not be found at the provided timestamp.
    ForexInvalidTimestamp,
    /// Returned when the forex base asset is found.
    ForexBaseAssetNotFound,
    /// Returned when the forex quote asset is found.
    ForexQuoteAssetNotFound,
    /// Returned when neither forex asset is found.
    ForexAssetsNotFound,
    /// Returned when the caller is not the CMC and there are too many active requests.
    RateLimited,
    /// Returned when the caller does not send enough cycles to make a request.
    NotEnoughCycles,
    /// Returned if too many collected rates deviate substantially.
    InconsistentRatesReceived,
    /// Until candid bug is fixed, new errors after launch will be placed here.
    Other(OtherError),
}

/// Used to provide details for the [ExchangeRateError::Other] variant field.
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct OtherError {
    /// The identifier for the error that occurred.
    pub code: u32,
    /// A description of the error that occurred.
    pub description: String,
}

/// Short-hand for returning the result of a `get_exchange_rate` request.
pub type GetExchangeRateResult = Result<ExchangeRate, ExchangeRateError>;

// user opens position
//the mount_in
//debt_value in collateral

#[derive(CandidType, Clone, Deserialize, Copy)]
pub struct FundingRateTracker {
    pub net_volume_long: Amount,
    pub total_long_shares: Amount,
    pub net_volume_short: Amount,
    pub total_short_shares: Amount,
}

impl FundingRateTracker {
    pub fn add_volume(&mut self, delta: Amount, long: bool) -> Amount {
        if long {
            let volume_share = _calc_shares(delta, self.total_long_shares, self.net_volume_long);
            self.total_long_shares += volume_share;
            self.net_volume_long += delta;
            return volume_share;
        } else {
            let volume_share = _calc_shares(delta, self.total_short_shares, self.net_volume_short);
            self.total_short_shares += volume_share;
            self.net_volume_short += delta;
            return volume_share;
        }
    }

    pub fn remove_volume(&mut self, delta: Amount, long: bool) -> Amount {
        if long {
            let value = _calc_shares_value(delta, self.total_long_shares, self.net_volume_long);
            self.net_volume_long -= value;
            self.total_long_shares -= delta;
            return value;
        } else {
            let value = _calc_shares_value(delta, self.total_short_shares, self.net_volume_short);
            self.net_volume_short -= value;
            self.total_short_shares -= delta;
            return value;
        }
    }

    pub fn settle_funding_rate(&mut self, funding_rate: u64, positive: bool) {
        if positive {
            let amount_to_settle = _percentage128(funding_rate, self.net_volume_long);
            self.net_volume_short += amount_to_settle;
            self.net_volume_long -= amount_to_settle;
        } else {
            let amount_to_settle = _percentage128(funding_rate, self.net_volume_short);
            self.net_volume_long += amount_to_settle;
            self.net_volume_short -= amount_to_settle
        }
    }
}

impl Storable for FundingRateTracker {
    const BOUND: Bound = Bound::Unbounded;
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

impl Default for FundingRateTracker {
    fn default() -> Self {
        FundingRateTracker {
            net_volume_long: 0,
            total_long_shares: 0,
            net_volume_short: 0,
            total_short_shares: 0,
        }
    }
}

///Market Details
#[derive(Clone, Deserialize, CandidType, Debug)]
pub struct MarketDetails {
    /// The details of the  perpetual asset also seen as the base asset  
    pub base_asset: Asset,

    /// the details of the collateral token  in asset  all margin is paid
    pub quote_asset: Asset,
    /// Vault ID
    ///
    /// The canister ID of the vault canister
    pub vault_id: Principal,

    pub xrc_id: Principal,

    /// token decimal of collateral token
    pub collateral_decimal: u8,
}

impl Default for MarketDetails {
    fn default() -> MarketDetails {
        return MarketDetails {
            base_asset: Asset::default(),
            quote_asset: Asset::default(),
            vault_id: Principal::anonymous(),
            xrc_id: Principal::anonymous(),
            collateral_decimal: 0,
        };
    }
}

impl Storable for MarketDetails {
    const BOUND: Bound = Bound::Unbounded;
    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

///State Details comprises for useful parameters that change more frequently
/// compared to market details  that is  set on initialization

#[derive(CandidType, Default, Debug, PartialEq, Eq, Copy, Deserialize, Clone)]
pub struct StateDetails {
    /// Determine if market is paused or not
    pub not_paused: bool,
    /// Current Tick
    ///
    ///
    pub current_tick: Tick,
    /// Max Leverage
    ///
    /// the maximum leverage allowed for any position * 10
    ///
    /// typically leverage is set multiplied by 10 ,so a leverage of 2x would be written as 20  
    pub max_leveragex10: u8,
    /// Minimum Collateral
    ///
    /// minimum collateral or minimum margin to open a position
    ///
    /// Note:
    ///
    /// -this amount  is actuallly  reduced by the reduction i.e (10::pow(token_decimal - 6))
    pub min_collateral: Amount,

    /// Base Token Multiplier
    ///
    /// base token multiple for cases of perp_assets with lower value than the underlying collateral asset
    pub base_token_multiple: u8,
}

impl Storable for StateDetails {
    const BOUND: Bound = Bound::Unbounded;
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

#[derive(CandidType, Default, Deserialize, Clone, Copy)]
pub struct TickDetails {
    pub liq_bounds_token0: LiquidityBoundary,

    pub liq_bounds_token1: LiquidityBoundary,

    pub created_timestamp: Time,
}

impl TickDetails {
    pub fn new() -> Self {
        TickDetails {
            liq_bounds_token0: LiquidityBoundary::default(),
            liq_bounds_token1: LiquidityBoundary::default(),
            created_timestamp: ic_cdk::api::time(),
        }
    }
    /// Add_liquidity function
    ///
    /// adds liquidity at current particular tick
    pub fn _add_liquidity(&mut self, buy: bool, amount_in: Amount) {
        if buy {
            self.liq_bounds_token1._add_liquidity(amount_in);
        } else {
            self.liq_bounds_token0._add_liquidity(amount_in);
        }
    }

    /// Reemove_liquidity function
    ///
    /// removes liquidity from the reference tick
    ///
    /// Note;This is only called while closing trade orders and retrieving static liqudiity
    /// it's not called  while closing liquidity orders
    pub fn _remove_liquidity(&mut self, buy: bool, amount_out: Amount) {
        if buy {
            self.liq_bounds_token1._remove_liquidity(amount_out);
        } else {
            self.liq_bounds_token0._remove_liquidity(amount_out);
        }
    }
}

/// Liquidity Boundary tracks the amount of Static Liquidity currently at a tick
///
///   Static Liquidity refers to liquidity from  limit orders that normal traders make
///
///   while Dynamic liquidity refers to liquidity provided by orders from liquidity providers  
///
///   Dynamic because it changes form with the same tick ,going from a buy order to a sell order and vice versa

#[derive(CandidType, Deserialize, Default, Copy, Clone, PartialEq, Eq)]
pub struct LiquidityBoundary {
    /// Upper Bound
    ///
    /// upper bound of all static liquidity put into the reference tick since it's (last time crossed)
    ///
    /// Note :this includes those closed or cancelled
    pub upper_bound: Amount,
    /// Lower Boound
    ///
    /// lower bound of all static liquidity put into the reference tick since it's (last time crossed)
    ///
    /// Note:
    ///
    ///  - Lower bound tracks the amouunt of asset static liquidity utilised
    ///
    ///  - the amouunt  of dynamic liquidity at a current tick is the upper bound - lower bound
    pub lower_bound: Amount,
    /// Lifetime  Removed Liquidity
    ///
    /// total amount of liquidity removed (by closing an order) at tick since initialisation
    pub lifetime_removed_liquidity: Amount,
}

impl LiquidityBoundary {
    /// Reduce boundary function
    ///
    /// reduces the boundary by adding amount and the total removed liquidity to lower bound
    ///
    /// setting the removed liquidity to zero
    pub fn _reduce_boundary(&mut self, amount: Amount) {
        self.lower_bound += amount;
    }

    /// LLiquidity within
    ///
    /// calculates liquidity within a boundary
    pub fn _liquidity_within(&self) -> Amount {
        return self.upper_bound - self.lower_bound;
    }
    /// Add Liqudity
    ///
    /// adds liquidity to boundary to a boundary by increasing the boundary upper bound by delta
    pub fn _add_liquidity(&mut self, delta: Amount) {
        self.upper_bound += delta;
    }

    /// Remove Liqudity
    ///
    /// removes liquidity from within a boundary be increasing removed liquidity
    pub fn _remove_liquidity(&mut self, delta: Amount) {
        self.lower_bound += delta;
        self.lifetime_removed_liquidity += delta
    }
}
