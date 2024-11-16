use super::constants::_PRICE_DECIMAL;

type Amount = u128;

pub fn _equivalent(amount: Amount, price: Amount, buy: bool) -> Amount {
    if buy {
        return (amount * _PRICE_DECIMAL) / price;
    } else {
        return (amount * price) / _PRICE_DECIMAL;
    }
}
