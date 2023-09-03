use scrypto::prelude::*;

// Calculate target amount from curve
pub fn calc_target(p0: Decimal, actual: Decimal, surplus: Decimal, k: Decimal) -> Decimal {
    let radicand = dec!(1) + dec!(4) * k * surplus / p0 / actual;
    let num = (dec!(2) * k - 1 + radicand.sqrt().unwrap()) * actual;
    num / k / dec!(2)
}

// Calculate spot price from curve
pub fn calc_spot(p0: Decimal, target: Decimal, actual: Decimal, k: Decimal) -> Decimal {
    let target2 = target * target;
    let actual2 = actual * actual;

    let num = actual2 + k * (target2 - actual2);
    num / actual2 * p0
}

// Calculate equilibrium price from shortage and spot price
pub fn calc_p0_from_spot(p_spot: Decimal, target: Decimal, actual: Decimal, k: Decimal) -> Decimal {
    let target2 = target * target;
    let actual2 = actual * actual;

    let den = actual2 + k * (target2 - actual2);
    actual2 / den * p_spot
}

// Calculate at what price incoming trades reach equilibrium following the curve
pub fn calc_p0_from_surplus(surplus: Decimal, target: Decimal, actual: Decimal, k: Decimal) -> Decimal {
    // Calculate the shortage of tokens
    let shortage = target - actual;

    // Calculate the price at equilibrium (p0) using the given formula
    surplus / shortage / (dec!(1) + k * shortage / actual)
}

// Calculate the incoming amount of output tokens given input_amount, target, actual, and p_ref
pub fn calc_incoming(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p_ref: Decimal,
    k_in: Decimal,
) -> Decimal {
    // Ensure the sum of the actual and input amounts does not exceed the target
    assert!(actual + input_amount <= target, "Infeasible amount");
    
    // Calculate the existing surplus as per AMM curve
    let surplus_before = (target - actual) * p_ref * (dec!(1) + k_in * (target - actual) / actual);

    // Calculate the new surplus as per the AMM curve
    let actual_after = actual + input_amount;
    let surplus_after = (target - actual_after) * p_ref * (dec!(1) + k_in * (target - actual_after) / actual_after);

    // The difference is the output amount
    surplus_before - surplus_after
}

// Calculate the amount of output tokens given input amount and current place on curve
pub fn calc_outgoing(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p_ref: Decimal,
    k_out: Decimal,
) -> Decimal {
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
                ).sqrt().unwrap()
            )
            / dec!(2)
            / (dec!(1) - k_out);

        // Calculate and return the difference in shortfall
        new_shortfall - shortfall
    }
}