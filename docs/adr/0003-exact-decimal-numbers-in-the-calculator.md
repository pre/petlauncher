# Exact decimal numbers with rust_decimal in the calculator

Status: accepted.

The calculator must give exact results for decimal numbers: `0.1+0.2` is `0.3`. It uses the 96-bit decimal type of the crate `rust_decimal` with the feature `maths`, and it marks each value as exact or not exact. An exact value shows all its digits. A value that is not exact shows 15 significant digits.

## Considered Options

- `f64`: `42.3+68.7+14.1+0.5` gives `125.60000000000001`.
- `bigdecimal`: any number of digits, but it has no `ln`, `exp` or trigonometric functions.
- `fastnum`: more digits and a larger range, but it is newer and less used.

## Consequences

- The range is about ±7.9e28 with at most 28 decimal places. `2^100` and `10^30` give "The number is too large".
- `asin`, `acos`, `atan` and the hyperbolic functions use `f64`, because the decimal type does not have them.
- `sin`, `cos` and `tan` do not accept an argument above 1e14, because their result has fewer than 15 correct digits there.
