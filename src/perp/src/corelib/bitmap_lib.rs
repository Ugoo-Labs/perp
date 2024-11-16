use super::bit_lib::{_least_sigbit_position, _most_sigbit_position};
use super::tick_lib::{_next_default_tick, _tick_zero};

use super::constants::{_ONE_BASIS_POINT, _ONE_PERCENT};

/// Flip Bit
///
/// This function is used to flip a particlar bit on a bitmap,
/// it either initialises it if it's not initialised or the reverse

pub fn _flip_bit(bitmap: u128, bit_position: u64) -> u128 {
    if bit_position == 0 {
        return bitmap;
    }
    let mask = 1 << (99 - bit_position);
    return bitmap ^ mask;
}

/// Next Initialised Tick
///
/// This function is used to calculate the next initialised tick from  the bitmap of an integral
///
/// Note
///  - This function returns the next default tick (see tick_lib) if no tick is initialised within the bitmap

pub fn _next_initialised_tick(bitmap: u128, integral: u64, bit_position: u64, buy: bool) -> u64 {
    let reference = 99 - bit_position;
    if buy {
        let mask = ((1u128) << reference) - 1;
        let masked = bitmap & mask;

        if masked == 0 {
            return _next_default_tick(integral, true); // (integral + 1) * _ONE_PERCENT;
        } else {
            return (integral * _ONE_PERCENT) + (_most_sigbit_position(masked) * _ONE_BASIS_POINT);
        }
    } else {
        let mask = !(((1u128) << (reference + 1)) - 1);
        let masked = mask & bitmap;

        if masked == 0 {
            if bit_position == 0 {
                return _next_default_tick(integral, false);
            }

            return _tick_zero(integral); // (integral - 1) * _ONE_PERCENT + (99 * _ONE_BASIS_POINT)
        } else {
            return (integral * _ONE_PERCENT) + (_least_sigbit_position(masked) * _ONE_BASIS_POINT);
        }
    }
}

#[test]

fn test_next_initialised_tick() {
    let bitmap = 8;
    let integral = 100;
    let next1 = _next_initialised_tick(bitmap, integral, 96, true);

    assert_eq!(next1, (integral + 1) * _ONE_PERCENT);

    let bitmap2 = 20;

    let next2 = _next_initialised_tick(bitmap2, integral, 95, true);

    assert_eq!(next2, (integral * _ONE_PERCENT) + (97 * _ONE_BASIS_POINT));

    let next3 = _next_initialised_tick(bitmap2, integral, 97, false);

    assert_eq!(next3, (integral * _ONE_PERCENT) + (95 * _ONE_BASIS_POINT));
}

#[cfg(test)]

mod unit_test {

    use super::*;
    #[test]

    fn test_flip_bit() {
        let bitmap = 8; //1000

        let val = _flip_bit(bitmap, 99);

        assert_eq!(val, 9);

        let val2 = _flip_bit(bitmap, 96);

        assert_eq!(val2, 0);
    }
}
