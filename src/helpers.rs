use scrypto::prelude::*;

// Calculate total amount of tokens in the pool at equilibrium when trading along the curve.
// Solves the corresponding quadratic equation explicitly.
// For well-formed input (all variables positive, 0<k<=1) the square root exists.
// Should not panic for any realistic input combination passing the assertions
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_target_ratio(p0: Decimal, actual: Decimal, surplus: Decimal, k: Decimal) -> Decimal {
    assert!(p0 > dec!(0), "Invalid p0");
    assert!(actual > dec!(0), "Invalid actual reserves");
    assert!(surplus >= dec!(0), "Invalid surplus amount");
    assert!(k >= dec!("0.001") && k <= dec!(1), "Invalid k");

    let radicand = dec!(1) + dec!(4) * k * surplus / p0 / actual;
    let num = dec!(2) * k - dec!(1) + radicand.checked_sqrt().unwrap();
    num / k / dec!(2)
}

// Calculate spot price for tokens (disregarding fee) when trading on the curve.
// Direct implementation of the Dodo price curve equation converted to ratio.
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_spot(p0: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(p0 > dec!(0), "Invalid p0");
    assert!(target_ratio >= dec!(1), "Invalid target ratio");
    assert!(k >= dec!("0.001") && k <= dec!(1), "Invalid k");

    let ratio2 = target_ratio * target_ratio;
    (dec!(1) + k * (ratio2 - dec!(1))) * p0
}

// Calculate equilibrium price from trading curve and known spot price.
// Based on direct implementation of Dodo spot price curve, rearranged to solve for p0.
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_p0_from_spot(p_spot: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(p_spot > dec!(0), "Invalid p_spot");
    assert!(target_ratio >= dec!(1), "Invalid target ratio");
    assert!(k >= dec!("0.001") && k <= dec!(1), "Invalid k");

    let ratio2 = target_ratio * target_ratio;
    p_spot / (dec!(1) + k * (ratio2 - dec!(1)))
}

// Calculate equilibrium price with the rest of the trading curve parameters given.
// Rearranges the integrated Dodo spot price curve to solve for p0.
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_p0_from_curve(shortfall: Decimal, surplus: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(shortfall > dec!(0), "Invalid shortfall");
    assert!(surplus > dec!(0), "Invalid surplus");
    assert!(target_ratio >= dec!(1), "Invalid target ratio");
    assert!(k >= dec!("0.001") && k <= dec!(1), "Invalid k");

    // Calculate the price at equilibrium (p0) using the given formula
    surplus / shortfall / (dec!(1) + k * (target_ratio - dec!(1)))
}

// Integrate along the trading curve towards equilibrium to find output corresponding to given input_amount.
// Works by applying the integrated spot price curve before and after the input_amount is added.
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_incoming(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p0: Decimal,
    k_in: Decimal,
) -> Decimal {
    // Ensure the sum of the actual and input amounts does not exceed the target
    assert!(input_amount > dec!(0), "Invalid input amount");
    assert!(target > actual, "Invalid target reserves");
    assert!(actual > dec!(0), "Invalid actual reserves");
    assert!(p0 > dec!(0), "Invalid reference price");
    assert!(k_in >= dec!("0.001") && k_in <= dec!(1), "Invalid k_in");
    assert!(actual + input_amount <= target, "Infeasible combination");
    
    // Calculate the expected surplus values
    let actual_after = actual + input_amount;
    let surplus_before = (target - actual) * p0 * (dec!(1) + k_in * (target - actual) / actual);
    let surplus_after = (target - actual_after) * p0 * (dec!(1) + k_in * (target - actual_after) / actual_after);

    // The difference is the output amount
    surplus_before - surplus_after
}

// Integrate along the trading curve away from equilibrium to find output corresponding to given input_amount.
// Works by explicit solution of the quadratic equation before and after the input_amount is added.
// For values of 0<=k<1 the square root exists and the outcome can be proven to be positive
// When k=1 the quadratic equation breaks down to 0/0 and we have a special (much simpler) solution.
// When k is very close to 1 we could have overflow, so k is limited to 0.999 or exactly 1 in the constructor.
// Potentially vulnerable to overflow for wildly unrealistic prices / amounts
pub fn calc_outgoing(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p_ref: Decimal,
    k_out: Decimal,
) -> Decimal {
    assert!(input_amount > dec!(0), "Invalid input amount");
    assert!(target >= actual, "Invalid target reserves");
    assert!(actual > dec!(0), "Invalid actual reserves");
    assert!(p_ref > dec!(0), "Invalid reference price");
    assert!(k_out >= dec!("0.001") && k_out <= dec!(1), "Invalid k_in");

    // Calculate current shortfall of tokens
    let shortfall = target - actual;

    // Calculate how many tokens should be in surplus according to the curve
    let surplus = shortfall / actual * (actual + k_out * shortfall) * p_ref;
    let scaled_new_surplus = (surplus + input_amount) / p_ref;

    // Special case for k_out equal to 1 (constant product)
    if k_out == dec!(1) {
        let new_shortfall = scaled_new_surplus * target / (target + scaled_new_surplus);

        // Calculate and return the difference in shortfall
        new_shortfall - shortfall
    } else {
        // Handle other values for k_out
        let new_shortfall =
            (
                target + scaled_new_surplus -
                (
                    target * target
                    + (dec!(4) * k_out - dec!(2)) * target * scaled_new_surplus
                    + scaled_new_surplus * scaled_new_surplus
                ).checked_sqrt().unwrap()
            )
            / dec!(2)
            / (dec!(1) - k_out);

        // Calculate and return the difference in shortfall
        new_shortfall - shortfall
    }
}