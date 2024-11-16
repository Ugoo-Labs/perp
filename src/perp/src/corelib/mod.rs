pub mod swap_lib;

pub mod order_lib;

pub mod bit_lib;
pub mod bitmap_lib;

pub mod calc_lib;

pub mod constants;

pub mod price_lib;

pub mod tick_lib;

#[cfg(test)]

mod unit_test {
    use super::*;
    use crate::types::{Amount, Tick, TickDetails};
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static MULTIPLIERS_BITMAPS:RefCell<HashMap<u64,u128>> = RefCell::new(HashMap::new());

        static TICKS_DETAILS :RefCell<HashMap<Tick,TickDetails>> = RefCell::new(HashMap::new());
    }

    #[test]
    fn test_place_order_and_swap() {
        // create a sell order at 200% price range

        let reference_tick = 199 * constants::_ONE_PERCENT;
        let mut order1 = order_lib::LimitOrder::new(1_000_000_000_000, reference_tick, true);
        _open_order(&mut order1);

        let amount_to_swap = 1_000_000_000;
        // swap_at_tick
        let (amount_out, _amount_remaining, _resulting_tick, _crossed_ticks) = _swap(
            amount_to_swap,
            false,
            230 * constants::_ONE_PERCENT,
            reference_tick,
        );

        println!("the amount out is {}", amount_out);

        //swap was executed at 200 percent ,so amount out is half of amoun to swap
        // assert_eq!(amount_out, amount_to_swap * 2);
        // // assert amount_remaining is zero
        // assert_eq!(amount_remaining, 0);
        // // assert resulting tick is reference tick
        // assert_eq!(resulting_tick, reference_tick);
        // // assert crossed_ticks.len  is zero
        // assert_eq!(crossed_ticks.len(), 0)

        //swap at that tick with large amount enough to totally remove order

        // remove order
    }

    #[test]
    fn test_place_order_swap_remove_order() {
        let reference_tick = 200 * constants::_ONE_PERCENT;
        let mut order1 = order_lib::LimitOrder::new(1_000_000_000_000, reference_tick, false);
        {
            _open_order(&mut order1);
        }

        let amount_to_swap = 1_000_000_000;
        // swap_at_tick
        {
            let (amount_out, _, _, _) = _swap(amount_to_swap, true, reference_tick, reference_tick);

            // remove order

            let (amount_filled, amount_remaining) = _close_order(&order1);
            // the amount filled is the total amount swapped during the swap transaction
            assert_eq!(amount_filled, amount_to_swap);

            // The amount still unfilled from the order is the inital amount for order minus the resulting amount from swapping
            assert_eq!(amount_remaining, order1.order_size - amount_out);
        }
    }

    #[test]

    // testing order
    fn test_first_come_first_cleared() {
        let reference_tick = 200 * constants::_ONE_PERCENT;
        let mut order1 = order_lib::LimitOrder::new(1_000_000_000_000, reference_tick, false);
        // open order 1
        {
            _open_order(&mut order1);
        }

        let mut order2 = order_lib::LimitOrder::new(10000000, reference_tick, false);
        //Open order 2
        {
            _open_order(&mut order2);
        }
        let amount_to_swap = 1_000_000_000;
        //swap
        let (amount_out, _, _, _) = _swap(amount_to_swap, true, reference_tick, reference_tick);

        //close order2
        {
            let (amount_filled, amount_remaining) = _close_order(&order2);
            // since amount to swap is not big enough to clear order1 ,order2 is not filled
            assert_eq!(amount_filled, 0);
            assert_eq!(amount_remaining, order2.order_size);
        }

        //close order1
        {
            let (amount_filled, amount_remaining) = _close_order(&order1);
            // since order 1 was put in first ,it is partially filled
            assert_eq!(amount_filled, amount_to_swap);
            assert_eq!(amount_remaining, order1.order_size - amount_out);
        }
    }
    ///
    ///
    ///
    ///
    ///
    fn _get_tick_details(tick: Tick) -> TickDetails {
        TICKS_DETAILS
            .with(|ref_tick_details| return ref_tick_details.borrow().get(&tick).unwrap().clone())
    }

    fn _open_order(order: &mut order_lib::LimitOrder) {
        TICKS_DETAILS.with(|ref_ticks_details| {
            let ticks_details = &mut *ref_ticks_details.borrow_mut();
            MULTIPLIERS_BITMAPS.with(|ref_multiplier_bitmaps| {
                let multipliers_bitmaps = &mut *ref_multiplier_bitmaps.borrow_mut();
                let mut open_order_params = order_lib::OpenOrderParams {
                    order,
                    integrals_bitmaps: multipliers_bitmaps,
                    ticks_details,
                };
                open_order_params.open_order();
            })
        });
    }

    ///
    ///
    fn _close_order(order: &order_lib::LimitOrder) -> (Amount, Amount) {
        TICKS_DETAILS.with(|ref_ticks_details| {
            let ticks_details = &mut *ref_ticks_details.borrow_mut();
            MULTIPLIERS_BITMAPS.with(|ref_multiplier_bitmaps| {
                let multipliers_bitmaps = &mut *ref_multiplier_bitmaps.borrow_mut();
                let mut close_order_params = order_lib::CloseOrderParams {
                    order,
                    multipliers_bitmaps,
                    ticks_details,
                };
                close_order_params.close_order()
            })
        })
    }

    fn _swap(
        order_size: Amount,
        buy: bool,
        init_tick: Tick,
        stopping_tick: Tick,
    ) -> (Amount, Amount, Tick, Vec<Tick>) {
        TICKS_DETAILS.with(|ref_ticks_details| {
            let ticks_details = &mut ref_ticks_details.borrow_mut();
            MULTIPLIERS_BITMAPS.with(|ref_multiplier_bitmaps| {
                let multipliers_bitmaps = &mut ref_multiplier_bitmaps.borrow_mut();
                let mut swap_params = swap_lib::SwapParams {
                    buy,
                    init_tick,
                    stopping_tick,
                    order_size,
                    integrals_bitmaps: multipliers_bitmaps,
                    ticks_details,
                };
                swap_params._swap()
            })
        })
    }
}
