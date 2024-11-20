use serde::{Deserialize, Serialize};

use super::bitmap_lib::_flip_bit;

use super::price_lib::_equivalent;
use super::tick_lib::{_int_and_dec, _tick_to_price};

use candid::CandidType;

use crate::types::TickDetails;

use ic_stable_structures::{memory_manager::VirtualMemory, DefaultMemoryImpl, StableBTreeMap};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type Time = u64;
type Tick = u64;
type Amount = u128;
type MB = StableBTreeMap<u64, u128, Memory>;
type TD = StableBTreeMap<u64, TickDetails, Memory>;

/// Order Trait for different OrderTypes
pub trait Order {
    fn _opening_update(&mut self, ref_tick_details: &mut TickDetails);
    fn _closing_update(&self, ref_tick_details: &mut TickDetails) -> (Amount, Amount);
}

///OpenOrderParams for creating orders

pub struct OpenOrderParams<'a> {
    /// Multiplier BitMaps
    ///
    ///A HashMap of  multipliers (percentiles) to their respective bitmap
    pub integrals_bitmaps: &'a mut MB,
    ///Ticks Details
    ///
    ///A HashMap of tick to their tick_details
    pub ticks_details: &'a mut TD,
    /// Order
    ///
    /// A mutable refrence to any generic type that implements the Order trait  determing which order type is being opened
    pub order: &'a mut LimitOrder,
}

impl<'a> OpenOrderParams<'a> {
    /// Open Order function
    ///
    /// creates an order at a particular tick
    pub fn open_order(&mut self) {
        let mut tick_details = match self.ticks_details.get(&self.order.ref_tick) {
            Some(details) => details,
            None => {
                let (integral, bit_position) = _int_and_dec(self.order.ref_tick);
                let map = match self.integrals_bitmaps.get(&integral) {
                    Some(_map) => _map,
                    None => 0,
                };

                let flipped_bitmap = _flip_bit(map, bit_position);

                self.integrals_bitmaps.insert(integral, flipped_bitmap);

                TickDetails::default()
            }
        };

        self.order._opening_update(&mut tick_details);
        self.ticks_details.insert(self.order.ref_tick, tick_details);
    }
}

///CloseOrderParams for closing order

pub struct CloseOrderParams<'a> {
    ///Order
    ///
    /// An immutable reference  to a  generic type order that implements the Order trait,
    /// determining which type of order is being closed
    pub order: &'a LimitOrder,

    /// Multipliers Bitmaps
    ///
    ///A HashMap of  multipliers (percentiles) to their respective bitmap
    pub integrals_bitmaps: &'a mut MB,
    ///Ticks Details
    ///
    ///A HashMap of tick to their tick_details
    pub ticks_details: &'a mut TD,
}

impl<'a> CloseOrderParams<'a> {
    /// Close_order function
    ///
    /// Returns a tuple
    ///
    /// Note
    /// - If closing a trade order ,tuple represents amount out vs amount remaining
    /// - If closing a liquidity order ,tuple represents token0 amount and token1 amount corresponding order shares(see LiquidityOrder)
    pub fn close_order(&mut self) -> (Amount, Amount) {
        match self.ticks_details.get(&self.order.ref_tick) {
            Some(mut tick_details) => {
                // if closing a trade  order ,this returns
                // amount_out and amount remaining
                // if closing a Liquidity order
                // it returns token0 amount and token1 amount
                let (amount0, amount1) = self.order._closing_update(&mut tick_details);

                //  if all liquidity is zero delete tick_details
                if tick_details.liq_bounds_token0._liquidity_within() == 0
                    && tick_details.liq_bounds_token1._liquidity_within() == 0
                {
                    self.ticks_details.remove(&self.order.ref_tick);

                    let (integral, bit_position) = _int_and_dec(self.order.ref_tick);

                    if let Some(bitmap) = self.integrals_bitmaps.get(&integral) {
                        let flipped_bitmap = _flip_bit(bitmap, bit_position);

                        self.integrals_bitmaps.insert(integral, flipped_bitmap);
                    }
                } else {
                    self.ticks_details.insert(self.order.ref_tick, tick_details);
                }
                return (amount0, amount1);
            }
            None => {
                // if tick details does not exist means all trade order  that currently references that tick
                //   has been filled
                let tick_price = _tick_to_price(self.order.ref_tick);
                return (
                    _equivalent(self.order.order_size, tick_price, self.order.buy),
                    0,
                );
            }
        };
    }
}

/// Trade Order for placing Limit Orders
#[derive(Default, CandidType, Copy, Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrder {
    /// true if order is a buy order
    pub buy: bool,
    /// order_size
    pub order_size: Amount,
    /// Initial Removed Liquidity
    ///
    /// The inital amount of liquidity already removed from tick before order was placed there
    ///
    pub init_removed_liquidity: Amount,
    /// Initial Lower Bound
    ///
    /// The initial upper bound
    /// Note
    ///  - The init upper bound is that of token1 for a buy order or that of token0 for a sell order
    pub init_lower_bound: Amount,
    /// Reference Tick
    ///
    /// the reference tick of the particular order
    pub ref_tick: Tick,

    // corresponds to the last timesamp when that tick was created
    pub init_tick_timestamp: Time,
}

impl LimitOrder {
    pub fn new(order_size: Amount, ref_tick: Tick, buy: bool) -> Self {
        return LimitOrder {
            order_size,
            ref_tick,
            buy,
            init_lower_bound: 0,
            init_removed_liquidity: 0,
            init_tick_timestamp: 0,
        };
    }
}

impl Order for LimitOrder {
    /// Opening Update function
    ///
    /// opens a trade order by
    ///  - Updating the reference tick details
    ///  - Updating the init_lower_bound and init_cross_time of the order
    fn _opening_update(&mut self, tick_details: &mut TickDetails) {
        let init_liq_bounds = if self.buy {
            tick_details.liq_bounds_token1
        } else {
            tick_details.liq_bounds_token0
        };

        self.init_tick_timestamp = tick_details.created_timestamp;

        self.init_lower_bound = init_liq_bounds.upper_bound;

        self.init_removed_liquidity = init_liq_bounds.lifetime_removed_liquidity;
        tick_details._add_liquidity(self.buy, self.order_size);
    }

    /// Closing Update function
    ///
    /// closing a trade order
    ///
    /// Returns
    /// - Amount Out :This returns the amount of the particular asset expected from the order
    /// i.e base asset(perp asset) for a buy order and quote asset (collateral asset) for a sell order
    /// - Amount Remaining :This  returns the amount  not filled in the order  

    fn _closing_update(&self, tick_details: &mut TickDetails) -> (Amount, Amount) {
        let tick_price = _tick_to_price(self.ref_tick);
        let equivalent = |amount: Amount| -> Amount { _equivalent(amount, tick_price, self.buy) };

        // this means order has been filled since tick hs been closed before
        if self.init_tick_timestamp < tick_details.created_timestamp {
            return (equivalent(self.order_size), 0);
        }

        let (tick_lower_bound, order_lower_bound) = if self.buy {
            (
                tick_details.liq_bounds_token1.lower_bound,
                self.init_lower_bound + tick_details.liq_bounds_token1.lifetime_removed_liquidity
                    - self.init_removed_liquidity,
            )
        } else {
            (
                tick_details.liq_bounds_token0.lower_bound,
                self.init_lower_bound + tick_details.liq_bounds_token0.lifetime_removed_liquidity
                    - self.init_removed_liquidity,
            )
        };

        let (amount_out, amount_remaining) =
            if tick_lower_bound < order_lower_bound + self.order_size {
                if tick_lower_bound < order_lower_bound {
                    //order not filled
                    (0, self.order_size)
                } else {
                    // order partially filled
                    (
                        equivalent(tick_lower_bound - order_lower_bound),
                        (order_lower_bound + self.order_size) - tick_lower_bound,
                    )
                }
            } else {
                // order fully filled
                (equivalent(self.order_size), 0)
            };

        tick_details._remove_liquidity(self.buy, amount_remaining);

        return (amount_out, amount_remaining);
    }
}

// #[cfg(test)]
// mod unit_test_order_lib {

//     use super::*;

//     use crate::types::{Amount, Tick, TickDetails};
//     use std::cell::RefCell;
//     use std::collections::HashMap;

//     thread_local! {
//         static MULTIPLIERS_BITMAPS:RefCell<HashMap<u64,u128>> = RefCell::new(HashMap::new());

//         static TICKS_DETAILS :RefCell<HashMap<Tick,TickDetails>> = RefCell::new(HashMap::new());
//     }

//     #[test]
//     fn test_place_order() {
//         let mut order1 = LimitOrder::new(10000000, 1000, true);
//         _open_order(&mut order1);
//         let tick_details = _get_tick_details(1000);

//         assert_eq!(order1.init_lower_bound, 0);

//         assert_eq!(tick_details.liq_bounds_token1.lifetime_removed_liquidity, 0);
//         assert_eq!(
//             tick_details.liq_bounds_token1.upper_bound,
//             order1.order_size,
//         );
//         //

//         // Second Order
//         let mut order2 = LimitOrder::new(1000000, 1000, true);
//         _open_order(&mut order2);

//         let tick_details = _get_tick_details(1000);

//         assert_eq!(order2.init_lower_bound, order1.order_size);

//         assert_eq!(
//             tick_details.liq_bounds_token1.upper_bound,
//             order1.order_size + order2.order_size,
//         )
//     }

//     #[test]

//     fn test_open_and_close_order() {
//         let mut order1 = LimitOrder::new(10000000, 1000, true);

//         //Open order
//         {
//             _open_order(&mut order1);
//             let tick_details = _get_tick_details(1000);
//             assert_eq!(order1.init_lower_bound, 0);

//             assert_eq!(tick_details.liq_bounds_token1.lifetime_removed_liquidity, 0);
//             assert_eq!(
//                 tick_details.liq_bounds_token1.upper_bound,
//                 order1.order_size,
//             );
//         }
//         //Close order
//         {
//             let (amount_out, amount_remaining) = _close_order(&order1);

//             assert_eq!(amount_out, 0);

//             assert_eq!(amount_remaining, order1.order_size);
//         }
//     }

//     #[test]
//     fn test_open_close_different_orders() {
//         let mut order1 = LimitOrder::new(10000000, 1000, true);
//         {
//             _open_order(&mut order1);
//             let tick_details = _get_tick_details(1000);

//             assert_eq!(order1.init_lower_bound, 0);

//             assert_eq!(tick_details.liq_bounds_token1.lifetime_removed_liquidity, 0);
//             assert_eq!(
//                 tick_details.liq_bounds_token1.upper_bound,
//                 order1.order_size,
//             );
//         }
//         //

//         // Second Order
//         let mut order2 = LimitOrder::new(1000000, 1000, true);
//         {
//             _open_order(&mut order2);

//             let tick_details = _get_tick_details(1000);

//             assert_eq!(order2.init_lower_bound, order1.order_size);

//             assert_eq!(
//                 tick_details.liq_bounds_token1.upper_bound,
//                 order1.order_size + order2.order_size,
//             );
//         }

//         let init_tick_details = _get_tick_details(1000);

//         // Remove Order
//         {
//             let (amount_out, amount_remaining) = _close_order(&order1);

//             assert_eq!(amount_out, 0);

//             assert_eq!(amount_remaining, order1.order_size);
//         }

//         let tick_details = _get_tick_details(1000);
//         //assert lifetime _removed liquidity is
//         assert_eq!(
//             tick_details.liq_bounds_token1.lifetime_removed_liquidity,
//             init_tick_details
//                 .liq_bounds_token1
//                 .lifetime_removed_liquidity
//                 + order1.order_size
//         );

//         assert_eq!(
//             tick_details.liq_bounds_token1._liquidity_within(),
//             order2.order_size
//         );
//     }

//     ///
//     ///
//     ///
//     fn _get_tick_details(tick: Tick) -> TickDetails {
//         TICKS_DETAILS
//             .with(|ref_tick_details| return ref_tick_details.borrow().get(&tick).unwrap().clone())
//     }

//     //

//     fn _open_order(order: &mut LimitOrder) {
//         TICKS_DETAILS.with(|ref_ticks_details| {
//             let ticks_details = &mut *ref_ticks_details.borrow_mut();
//             MULTIPLIERS_BITMAPS.with(|ref_multiplier_bitmaps| {
//                 let multipliers_bitmaps = &mut *ref_multiplier_bitmaps.borrow_mut();

//                 let mut open_order_params = OpenOrderParams {
//                     order,
//                     integrals_bitmaps: multipliers_bitmaps,
//                     ticks_details,
//                 };
//                 open_order_params.open_order();
//             })
//         });
//     }

//     ///
//     ///
//     ///
//     ///
//     fn _close_order(order: &LimitOrder) -> (Amount, Amount) {
//         TICKS_DETAILS.with(|ref_ticks_details| {
//             let ticks_details = &mut *ref_ticks_details.borrow_mut();
//             MULTIPLIERS_BITMAPS.with(|ref_multiplier_bitmaps| {
//                 let multipliers_bitmaps = &mut *ref_multiplier_bitmaps.borrow_mut();

//                 let mut close_order_params = CloseOrderParams {
//                     order,
//                     integrals_bitmaps: multipliers_bitmaps,
//                     ticks_details,
//                 };
//                 close_order_params.close_order()
//             })
//         })
//     }
// }
