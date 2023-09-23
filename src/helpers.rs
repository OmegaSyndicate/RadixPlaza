use scrypto::prelude::*;
use crate::constants::*;

/// Helper function `deposit_to_pool` enables a deposit transaction to occur from a `bucket` to the `pool`. The
/// deposit is only carried out if the `amount` exceeds zero. 
///
/// Arguments:
/// * `pool`: The global of type `TwoResourcePool`, where the deposit will be placed.
/// * `bucket`: The user's bucket, the source of the deposit.
/// * `amount`: The quantity to be transferred from `bucket` to `pool`.
///
/// The deposit operation will only take place if the `amount` is strictly positive.
pub fn deposit_to_pool(pool: &mut Global<TwoResourcePool>, bucket: &mut Bucket, amount: Decimal) {
    if amount > ZERO {
        pool.protected_deposit(bucket.take(amount));
    }
}

/// Helper function `withdraw_from_pool` enables a withdrawal operation from a `pool` to a `bucket`. The withdrawal is 
/// only conducted if the specified `amount` is greater than zero.
///
/// Arguments:
/// * `pool`: The global of type `TwoResourcePool` from which the withdrawal is executed.
/// * `bucket`: The user's bucket where the withdrawn tokens will be placed.
/// * `amount`: The quantity of tokens to be withdrawn from `pool` to `bucket`.
///
/// A withdrawal operation will only be performed if the `amount` is strictly positive.
pub fn withdraw_from_pool(pool: &mut Global<TwoResourcePool>, bucket: &mut Bucket, amount: Decimal) {
    let address = bucket.resource_address();
    if amount > ZERO {
        bucket.put(pool.protected_withdraw(address, amount, WithdrawStrategy::Exact));
    }
}
