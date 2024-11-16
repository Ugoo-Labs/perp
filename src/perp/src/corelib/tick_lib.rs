use super::calc_lib::{_percentage128, _percentage64};
use super::constants::*;

/// Default Max Tick
///
/// Gets the default max tick for a particular trade direction (buy or sell)
///
/// This is currently implemented as a 5 percent incerase or decrease from the current tick

pub fn _def_max_tick(current_tick: u64, buy: bool) -> u64 {
    if buy {
        current_tick + _percentage64(5 * _ONE_PERCENT, current_tick)
    } else {
        current_tick - _percentage64(5 * _ONE_PERCENT, current_tick)
    }
}

/// Next Default Tick
///
///
pub fn _next_default_tick(integral: u64, buy: bool) -> u64 {
    if buy {
        _tick_zero(integral + 1)
    } else {
        _tick_zero(integral - 1) + (99 * _ONE_BASIS_POINT)
    }
}

/// Tick Zero
///
/// The tick zero of an integral corresponds to the tick with that integral  and a  of 0 i.e whole percentages (1%,3% etc)
pub fn _tick_zero(integral: u64) -> u64 {
    integral * _ONE_PERCENT
}

/// Mul and Bit
///
/// This function is used to calculate the integral and decimal pert of a tick

pub fn _int_and_dec(tick: u64) -> (u64, u64) {
    let multiplier = tick / _ONE_PERCENT;
    let bit_position = (tick % _ONE_PERCENT) / (_ONE_BASIS_POINT);
    return (multiplier, bit_position);
}

/// Excceded Stopping Tick
///
/// This functions checks that stoping tick is not exceeded in the particular swap direction
///
///  

pub fn _exceeded_stopping_tick(current_tick: u64, stopping_tick: u64, buy: bool) -> bool {
    if buy {
        return current_tick > stopping_tick;
    } else {
        return current_tick < stopping_tick;
    }
}

/// Tick to Price
///
/// Calculates the price for a particular tick
/// Price is given as the percentage of a base price

pub fn _tick_to_price(tick: u64) -> u128 {
    return _percentage128(tick, _BASE_PRICE);
}

#[cfg(test)]

mod unit_test {

    use super::*;
    #[test]

    fn test_mul_and_bit() {
        let tick = 199_20_000;

        println!(
            "the number is {}",
            0.0000000000000002 * (10 as u128).pow(20) as f64
        );

        let (mul, bit) = _int_and_dec(tick);

        assert_eq!(mul, 199);
        assert_eq!(bit, 20);

        let tick2 = 199_200_000;

        let (mul2, bit2) = _int_and_dec(tick2);

        assert_eq!(mul2, 1992);
        assert_eq!(bit2, 0);
    }
}
