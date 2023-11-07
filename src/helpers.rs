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
        bucket.put(pool.protected_withdraw(address, amount, WithdrawStrategy::Rounded(RoundingMode::ToZero)));
    }
}

/// Function `assure_is_not_recallable` carries out a validation operation to ensure a token is not recallable. The 
/// validation is dependent on the `token` not having the 'recaller' or 'recaller_updater' roles active.
///
/// Arguments:
/// * `token`: The token of type `ResourceAddress` that needs its recallability status validated.
///
/// In the eventuality of either the 'recaller' or 'recaller_updater' role being activated for the token, the function
/// triggers assertion errors, effectively preventing the validation of tokens that may be recallable.
///
/// This function does not return any value.
pub fn assure_is_not_recallable(token: ResourceAddress) {
    let manager = ResourceManager::from(token);
    let token_recaller_rule = manager.get_role("recaller").unwrap();
    let token_recaller_updater_rule = manager.get_role("recaller_updater").unwrap();
    let target_rule = rule!(deny_all);

    assert_eq!(
        token_recaller_rule, 
        target_rule,
        "Cannot accept recallable tokens"
    );

    assert_eq!(
        token_recaller_updater_rule, 
        target_rule,
        "Cannot accept potentially recallable tokens"
    );
}
