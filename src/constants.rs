use scrypto::prelude::*;

pub const ZERO: Decimal = Decimal::zero();                                                      // zero
pub const ONE: Decimal = Decimal::one();                                                        // one
pub const TWO: Decimal = Decimal(I192::from_digits([2*10_u64.pow(18), 0, 0]));                  // two
pub const MIN_LIQUIDITY: Decimal = Decimal(I192::from_digits([10_u64.pow(18-6), 0, 0]));        // 10^-6
pub const MIN_K_IN: Decimal = Decimal(I192::from_digits([10_u64.pow(18-3), 0, 0]));             // 10^-3
pub const CLIP_K_OUT: Decimal = Decimal(I192::from_digits([999 * 10_u64.pow(18-3), 0, 0]));     // 0.999