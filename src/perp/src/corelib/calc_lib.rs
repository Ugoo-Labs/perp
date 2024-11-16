use ic_cdk::api::time;

use super::constants::*;

type Amount = u128;

/// Calculate Interest Function
///
/// This function calculates the interest on a leveraged position since when it was filled
/// The interest is calculated on an hourly basis
///
/// Note:Interest only counts if position is older than one hour  

pub fn _calc_interest(debt: Amount, interest_rate: u32, start_time: u64) -> Amount {
    let mut fee: Amount = 0;

    let one_hour: u64 = 3600 * ((10u64).pow(9));

    let mut _starting_time = start_time;

    let current_time = time();

    while start_time + one_hour < current_time {
        fee += ((interest_rate as u128) * debt) / u128::from(100 * _ONE_PERCENT);

        _starting_time += one_hour;
    }

    return fee;
}

/// Calculates Shares
///
/// This function calculates the amount of shares given the amount of asset being put in ,the current total shares and the current net liquidity

pub fn _calc_shares(
    amount_in: Amount,
    init_total_shares: Amount,
    init_liquidity: Amount,
) -> Amount {
    if init_total_shares == 0 {
        return amount_in;
    }
    return (amount_in * init_total_shares) / init_liquidity;
}

/// Calculate Shares Value
///
/// This function calculates the value of a particular share given the current  amount of shares  and the  current net liquidity
pub fn _calc_shares_value(
    shares: Amount,
    init_total_shares: Amount,
    init_liquidity: Amount,
) -> Amount {
    return (shares * init_liquidity) / init_total_shares;
}

/// Percentage Functions
///
/// These functions  calculates percentages  

pub fn _percentage128(x: u64, value: Amount) -> Amount {
    return ((x as u128) * value) / (100 * _ONE_PERCENT as u128);
}

pub fn _percentage64(x: u64, value: u64) -> u64 {
    return (x * value) / (100 * _ONE_PERCENT);
}
