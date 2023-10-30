use scrypto::prelude::*;
use scrypto::types::RoyaltyAmount::Usd;

pub const ZERO: Decimal = Decimal::zero();                          // zero
pub const ONE_TENTH: Decimal = dec!(0.1);                           // 10^-1
pub const ONE: Decimal = Decimal::one();                            // one
pub const TWO: Decimal = dec!(2);                                   // two
pub const FOUR: Decimal = dec!(4);                                  // four
pub const ONE_HUNDRED: Decimal = dec!(100);                         // 100
pub const MIN_LIQUIDITY: Decimal = dec!(0.0001);                    // 10^-4
pub const FEMTO: Decimal = Decimal(I192::from_digits([1000, 0, 0]));// 10^-15
pub const MIN_K_IN: Decimal = dec!(0.001);                          // 10^-3
pub const CLIP_K_OUT_1: Decimal = dec!(0.999);                      // 0.999
pub const CLIP_K_OUT_2: Decimal = dec!(1.001);                      // 1.001
pub const _SWAP_ROYALTY: RoyaltyAmount = Usd(dec!(0.05));           // $0.05