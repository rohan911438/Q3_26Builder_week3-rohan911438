/// Integer square root (floor) via Newton's method. Used only to size the
/// very first liquidity deposit into a pool — see `add_liquidity`.
pub fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }

    let mut x = value;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_of_zero_is_zero() {
        assert_eq!(integer_sqrt(0), 0);
    }

    #[test]
    fn sqrt_of_perfect_square() {
        assert_eq!(integer_sqrt(4), 2);
        assert_eq!(integer_sqrt(2_000_000), 1414); // floor(sqrt(2_000_000))
    }

    #[test]
    fn sqrt_rounds_down_for_non_perfect_squares() {
        assert_eq!(integer_sqrt(3), 1);
        assert_eq!(integer_sqrt(99), 9);
    }
}
