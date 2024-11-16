use std::u128;

///BitLib library for calculating bit position of either most significant or least significant bit in a number
///
/// This Library complements the Bitmap Lib  and is prone to error if  used outside that context

///Most Significant Bit Position
///
/// Calculates the  position(within the range of 1-99) of the most significant one (1) in the binary representation of a NON zero number
pub fn _most_sigbit_position(num: u128) -> u64 {
    return (num.leading_zeros() as u64) - 28;
}

/// Least Significant Bit Position
///
/// Calculates the position(within the range of 1-99) of the least significant one (1) inthe binary representation of a NON zero number

pub fn _least_sigbit_position(num: u128) -> u64 {
    // This migjt result in a bug for number's with much zero but this is prevented in the implementation
    //of next_initialised tick as num can not be larger than 99 hence preventing a error
    return 99 - (num.trailing_zeros() as u64);
}
