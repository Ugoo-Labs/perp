use super::bitmap_lib::{_flip_bit, _next_initialised_tick};
use super::price_lib::_equivalent;
use super::tick_lib::*;
use crate::types::TickDetails;

use ic_stable_structures::{memory_manager::VirtualMemory, DefaultMemoryImpl, StableBTreeMap};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type Tick = u64;
type Amount = u128;
type MB = StableBTreeMap<u64, u128, Memory>;
type TD = StableBTreeMap<u64, TickDetails, Memory>;

/// Get Best Offer
///
///Gets the best offer for either selling or buying

pub fn _get_best_offer<'a>(
    buy: bool,
    current_tick: Tick,
    stopping_tick: Tick,
    integrals_bitmaps: &'a mut MB,
    ticks_details: &'a mut TD,
) -> Option<Tick> {
    let mut resulting_tick = 0;
    let mut loop_current_tick = current_tick;
    while !(_exceeded_stopping_tick(loop_current_tick, stopping_tick, buy)) {
        let (integral, bit_position) = _int_and_dec(loop_current_tick);
        let bitmap = match integrals_bitmaps.get(&integral) {
            Some(res) => res,
            None => {
                // if integral has no bitmap means that means  no tick within that integral   is
                //initialised

                // calculates the  next_default tick (See bitmap_lib)
                // if next default tick exceeds stopping tick
                //breaks else
                // updates current tick to the next default tick

                let next_default_tick = _next_default_tick(integral, buy);

                loop_current_tick = next_default_tick;
                //stops currrent iteration,starts the next at the next default tick
                continue;
            }
        };
        let tick_details = match ticks_details.get(&loop_current_tick) {
            Some(res) => res,
            None => TickDetails::default(),
        };
        let liquidity_boundary = if buy {
            tick_details.liq_bounds_token0
        } else {
            tick_details.liq_bounds_token1
        };

        if liquidity_boundary._liquidity_within() > 0 {
            resulting_tick = loop_current_tick;
            break;
        }

        let next_initialised_tick = _next_initialised_tick(bitmap, integral, bit_position, buy);

        loop_current_tick = next_initialised_tick;
    }

    if resulting_tick == 0 {
        return None;
    } else {
        return Some(resulting_tick);
    }
}

struct SwapTickConstants {
    tick: Tick,
    order_size: Amount,
}

/// SwapParams for initiating  a swap
/// utilsed for opening position at market price
pub struct SwapParams<'a> {
    /// Swap Direction
    ///
    /// true if buying or false if selling
    pub buy: bool,
    /// Init Tick
    ///
    /// the current state tick of the market ,also seen as current price
    ///
    /// this also translates to the current price
    pub init_tick: Tick,
    ///Stopping Tick
    ///
    /// stopping tick at which swapping should not not exceed
    ///
    /// This can be viewed as maximum excecution price for a market order
    /// if specified swap does not exceed this and returns the net previous amount from ticks below and the amount remaining
    pub stopping_tick: Tick,
    /// Order Size
    ///
    /// the amount of asset being swapped
    pub order_size: Amount,
    /// Multiplier BitMaps
    ///
    /// HashMap  of integrals to their bitmaps
    pub integrals_bitmaps: &'a mut MB,
    /// Ticks Details
    ///
    /// HashMasp  of ticks to their  respective tick_details
    pub ticks_details: &'a mut TD,
}

impl<'a> SwapParams<'a> {
    /// Swap Function
    ///
    /// Swap is executed as a loop starting at the current tick till stopping tick is reached is exceeded
    ///
    /// Returns
    ///  - AmountOut :The amount of token gotten from the swap
    ///  - AmountRemaining :The amount of asset remaining dues to swap not being completely filled before stopping tick
    ///  - Current or Resulting Tick : This corresponds to the tick at which either asset was fully swapped
    /// or tick before stopping tick was exceeded
    ///
    ///  - Crossed Ticks :The Total ticks that were crossed
    pub fn _swap(&mut self) -> (Amount, Amount, Tick, Vec<Tick>) {
        let mut amount_out = 0;

        let mut amount_remaining = self.order_size;

        let mut resulting_tick = self.init_tick;

        let mut crossed_ticks: Vec<Tick> = Vec::new();

        let mut loop_current_tick = self.init_tick;

        while !(_exceeded_stopping_tick(loop_current_tick, self.stopping_tick, self.buy)) {
            let (integral, bit_position) = _int_and_dec(loop_current_tick);

            let bitmap = match self.integrals_bitmaps.get(&integral) {
                Some(res) => res,
                None => {
                    // if integral has no bitmap means that means  no tick within that integral   is
                    //initialised

                    // calculates the  next_default tick (See bitmap_lib)
                    // if next default tick exceeds stopping tick
                    //breaks else
                    // updates current tick to the next default tick

                    let next_default_tick = _next_default_tick(integral, self.buy);
                    if _exceeded_stopping_tick(next_default_tick, self.stopping_tick, self.buy) {
                        break;
                    };

                    loop_current_tick = next_default_tick;
                    //stops currrent iteration,starts the next at the next default tick
                    continue;
                }
            };

            let tick_params = SwapTickConstants {
                order_size: amount_remaining,
                tick: loop_current_tick,
            };

            let (value_out, boundary_closed);

            if self.buy {
                (value_out, amount_remaining, boundary_closed) = self._buy_at_tick(tick_params);
            } else {
                (value_out, amount_remaining, boundary_closed) = self._sell_at_tick(tick_params);
            }

            // if static liquidity was exhausted at that tick and val out is greater to zero

            if value_out > 0 {
                amount_out += value_out;

                resulting_tick = loop_current_tick;

                // if static liquidity was exhausted at that tick and val out is not equal to zero

                if boundary_closed {
                    self.ticks_details.remove(&loop_current_tick);
                    //add ticks to list of crossed ticks
                    crossed_ticks.push(loop_current_tick);

                    let flipped_bitmap = _flip_bit(bitmap, bit_position);

                    let tick_zero = _tick_zero(integral);
                    // if flipping bitmap results in zero and tick zero(see bitmap_lib) is not contained in ticks_details hashmap
                    //delete btimap
                    if flipped_bitmap == 0 && !self.ticks_details.contains_key(&tick_zero) {
                        self.integrals_bitmaps.remove(&integral);
                    } else {
                        // insert flipped bitmap
                        self.integrals_bitmaps.insert(integral, flipped_bitmap);
                    };
                }

                if amount_remaining == 0 {
                    break;
                }
            }

            //println!()
            let next_initialised_tick =
                _next_initialised_tick(bitmap, integral, bit_position, self.buy);

            loop_current_tick = next_initialised_tick;
        }
        // if swap could not happen ,current tick remains unchanged and can only be changed manually

        return (amount_out, amount_remaining, resulting_tick, crossed_ticks);
    }

    /// buy at tick function
    ///
    /// Performs a swap at a particular tick
    ///
    /// Returns
    /// - AmountOut :The amount resulting from the swap for a buy order at that tick
    /// - AmountRemaining :The amount remaining from swapping at that tick ,
    ///  this is  zero if the swap is completedly fully at tick
    /// - Cleared : true if all liquidity at tick  was cleared
    /// - Boundary Closed : true if all static liquidity at tick (see TickDetails and LiquidityBoundary) is cleared
    fn _buy_at_tick(&mut self, params: SwapTickConstants) -> (Amount, Amount, bool) {
        let mut amount_out = 0;

        let mut amount_remaining = params.order_size;

        let boundary_closed;

        let tick_price = _tick_to_price(params.tick);

        let equivalent =
            |amount: Amount, buy: bool| -> Amount { _equivalent(amount, tick_price, buy) };

        let mut tick_details = match self.ticks_details.get(&params.tick) {
            Some(res) => res,
            None => return (amount_out, amount_remaining, false),
        };

        let init_tick_liq = tick_details.liq_bounds_token0._liquidity_within();

        //value of all_liquidity in token1
        let init_liq_equivalent = equivalent(init_tick_liq, false);

        if init_liq_equivalent <= self.order_size {
            // all liquidity has been exhausted
            amount_out = init_tick_liq;

            amount_remaining -= init_liq_equivalent;
        } else {
            //liquidity remains
            amount_out = equivalent(self.order_size, true);

            amount_remaining = 0;
        }

        // reduce static liquidity by amount out
        tick_details.liq_bounds_token0._reduce_boundary(amount_out);

        boundary_closed = tick_details.liq_bounds_token0._liquidity_within() == 0
            && tick_details.liq_bounds_token1._liquidity_within() == 0;

        self.ticks_details.insert(params.tick, tick_details);

        return (amount_out, amount_remaining, boundary_closed);
    }

    /// Sell at tick function
    ///
    /// Performs a swap at a particular tick
    ///
    /// Returns
    /// - AmountOut :The amount resulting from the swap for a sell order at that tick
    /// - AmountRemaining :The amount remaining from swapping at that tick ,
    ///  this is  zero if the swap is completedly fully at tick
    /// - Cleared : true if all liquidity at tick  was cleared
    /// - Boundary Closed : true if all static liquidity at tick (see TickDetails and LiquidityBoundary) is cleared

    fn _sell_at_tick(&mut self, params: SwapTickConstants) -> (Amount, Amount, bool) {
        let mut amount_out = 0;

        let mut amount_remaining = params.order_size;

        let boundary_closed;

        let tick_price = _tick_to_price(params.tick);

        let equivalent =
            |amount: Amount, buy: bool| -> Amount { _equivalent(amount, tick_price, buy) };

        // tick details
        let mut tick_details = match self.ticks_details.get(&params.tick) {
            Some(res) => res,
            None => return (amount_out, amount_remaining, false),
        };

        let init_tick_liq = tick_details.liq_bounds_token1._liquidity_within();

        let init_liq_equivalent = equivalent(init_tick_liq, true);

        if init_liq_equivalent <= self.order_size {
            amount_out = init_tick_liq;

            amount_remaining -= init_liq_equivalent;
        } else {
            //liquidity remains
            amount_out = equivalent(self.order_size, false);

            amount_remaining = 0;
        }

        tick_details.liq_bounds_token1._reduce_boundary(amount_out);

        boundary_closed = tick_details.liq_bounds_token1._liquidity_within() == 0
            && tick_details.liq_bounds_token0._liquidity_within() == 0;

        self.ticks_details.insert(params.tick, tick_details);

        return (amount_out, amount_remaining, boundary_closed);
    }
}

// #[cfg(test)]
// mod test {

//     //use crate::types::LiquidityBoundary;

//     use super::*;

//     // use super::{_mul_and_bit, _tick_zero};
//     use std::cell::RefCell;
//     use std::collections::HashMap;

//     thread_local! {
//         static INTEGRALS_BITMAPS:RefCell<MB> = RefCell::new(HashMap::new());
//         static TICKS_DETAILS:RefCell<TD> = RefCell::new(HashMap::new());
//     }

//     #[test]
//     fn test_exceeded() {
//         let order_size = 10000000000;
//         let starting_tick = 199_00_000;

//         let stopping_tick = _def_max_tick(starting_tick, true);

//         let (amount_out, amount_remaining, resulting_tick, crossed_ticks) =
//             _swap(order_size, true, starting_tick, stopping_tick);

//         assert_eq!(amount_out, 0);

//         assert_eq!(amount_remaining, order_size);
//         assert_eq!(resulting_tick, starting_tick);
//         assert_eq!(crossed_ticks.len(), 0);
//     }

//     #[test]

//     fn test_swap_at_tick() {
//         // test_swap_clearing_tick
//         {
//             let swapping_tick = 200_00_000;
//             let amount_at_tick = 200_000;

//             _fill_tick(swapping_tick, amount_at_tick, false);

//             let order_size: u128 = 10000000000000000000;

//             let (amount_out, amount_remaining, resulting_tick, _crossed_ticks) =
//                 _swap(order_size, true, swapping_tick, swapping_tick);

//             assert_eq!(amount_out, amount_at_tick);
//             assert_eq!(resulting_tick, swapping_tick);
//             assert_eq!(amount_remaining, order_size - (2 * amount_at_tick));

//             // assert that tick has been cleared and integral's bitmap has also  been cleared
//             assert_eq!(
//                 TICKS_DETAILS
//                     .with_borrow(|ticks_details| { ticks_details.contains_key(&swapping_tick) }),
//                 false
//             );

//             let (int, _) = _int_and_dec(swapping_tick);

//             assert_eq!(
//                 INTEGRALS_BITMAPS.with_borrow(|int_bitmaps| { int_bitmaps.contains_key(&int) }),
//                 false
//             );
//         }

//         // test_swap_tick
//         {
//             let swapping_tick = 200_00_000;
//             let amount_at_tick = 200_000_000;

//             _fill_tick(swapping_tick, amount_at_tick, true);

//             let order_size = 100_000;

//             let (amount_out, amount_remaining, resulting_tick, _crossed_ticks) =
//                 _swap(order_size, false, swapping_tick, 199_00_000);

//             assert_eq!(amount_out, order_size * 2); //trade was executed at 200 percent
//             assert_eq!(amount_remaining, 0);
//             assert_eq!(resulting_tick, swapping_tick);

//             // assert that tick has not  been cleared and integral's bitmap has also not  been cleared
//             assert_eq!(
//                 TICKS_DETAILS
//                     .with_borrow(|ticks_details| { ticks_details.contains_key(&swapping_tick) }),
//                 true
//             );

//             let (int, _) = _int_and_dec(swapping_tick);

//             assert_eq!(
//                 INTEGRALS_BITMAPS.with_borrow(|int_bitmaps| { int_bitmaps.contains_key(&int) }),
//                 true
//             );
//         }
//     }

//     #[test]
//     fn test_swap_across_tick() {
//         {
//             let (tick1, tick2) = (199_00_000, 199_50_000);
//             let (amount_at_tick1, amount_at_tick2) = (10_000_000_000_000, 200_000_000_000);

//             _fill_tick(tick1, amount_at_tick1, true);

//             _fill_tick(tick2, amount_at_tick2, true);

//             let amount_to_swap = 250_000_000_000;
//             let current_tick = 200_80_000;
//             let (amount_out, amount_remaining, resulting_tick, crossed_ticks) =
//                 _swap(amount_to_swap, false, current_tick, 190_00_000); // randoom stopping tick

//             println!("the value out is {}", amount_out);

//             assert!(amount_out > amount_at_tick2);
//             assert_eq!(amount_remaining, 0);
//             assert_eq!(resulting_tick, tick1);
//             assert_eq!(crossed_ticks.len(), 1);

//             // assert that the tick details has been deleted
//             assert_eq!(
//                 TICKS_DETAILS.with_borrow(|ticks_details| { ticks_details.contains_key(&tick2) }),
//                 false
//             );

//             // assert that integral'ss bitmap is still available since
//             let (int, _) = _int_and_dec(tick1);
//             assert!(INTEGRALS_BITMAPS
//                 .with_borrow(|integrals_bitmaps| { integrals_bitmaps.contains_key(&int) }))
//         }
//     }

//     fn _get_tick_details(tick: Tick) -> TickDetails {
//         TICKS_DETAILS.with_borrow(|ticks_details| ticks_details.get(&tick).unwrap().clone())
//     }

//     fn _fill_tick(tick: Tick, amount_in: Amount, buy: bool) {
//         let mut tick_details = TickDetails::new();
//         if buy {
//             tick_details.liq_bounds_token1._add_liquidity(amount_in);
//         } else {
//             tick_details.liq_bounds_token0._add_liquidity(amount_in);
//         }
//         let (int, dec) = _int_and_dec(tick);
//         TICKS_DETAILS
//             .with_borrow_mut(|ref_tick_details| ref_tick_details.insert(tick, tick_details));
//         INTEGRALS_BITMAPS.with_borrow_mut(|ref_integral_bitmaps| {
//             let bitmap = ref_integral_bitmaps.entry(int).or_insert(0);
//             let new_bitmap = _flip_bit(*bitmap, dec);

//             println!("the new bitmap is {}", new_bitmap);
//             *bitmap = new_bitmap;
//         })
//     }

//     #[test]
//     fn test_bitmap() {}

//     fn _swap(
//         order_size: Amount,
//         buy: bool,
//         init_tick: Tick,
//         stopping_tick: Tick,
//     ) -> (Amount, Amount, Tick, Vec<Tick>) {
//         TICKS_DETAILS.with_borrow_mut(|ticks_details| {
//             INTEGRALS_BITMAPS.with_borrow_mut(|integrals_bitmaps| {
//                 let mut swap_params = SwapParams {
//                     buy,
//                     init_tick,
//                     stopping_tick,
//                     order_size,
//                     integrals_bitmaps,
//                     ticks_details,
//                 };
//                 swap_params._swap()
//             })
//         })
//     }
// }
