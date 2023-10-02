use scrypto::prelude::*;
use crate::constants::*;

/// This function determines the target ratio of tokens in a liquidity pool at equilibrium by solving a quadratic
/// equation derived from parameters of the trading curve. 
///
/// ### Parameters
/// * `p0`: The initial token price. Must be a positive Decimal or the function will panic.
/// * `actual`: The current reserves of tokens. Must be a positive Decimal or the function will panic.
/// * `surplus`: The surplus of tokens. Must be a Decimal greater than or equal to ZERO or the function will panic.
/// * `k`: The pool's curvature rate. Must be a Decimal within the range of MIN_K_IN and ONE inclusively, or the
///    function will panic.
///
/// ### Function equation
/// The function calculates the following equation: 
/// radicand = 1 + 4*k*surplus/(p0*actual),
/// num = 2*k - 1 + sqrt(radicand),
/// target_ratio = num/(2*k).
///
/// ### Returns
/// The function returns a `Decimal` value representing the target ratio of tokens at equilibrium state. 
///
/// ### Panics
/// The function can cause a panic if any of the input assertions fail. It may panic due to overflow from extreme
/// combinations of input parameters.
pub fn calc_target_ratio(p0: Decimal, actual: Decimal, surplus: Decimal, k: Decimal) -> Decimal {
    assert!(p0 > ZERO, "Invalid p0");
    assert!(actual > ZERO, "Invalid actual reserves");
    assert!(surplus >= ZERO, "Invalid surplus amount");
    assert!(k >= MIN_K_IN, "Invalid k");

    let radicand = ONE + FOUR * k * surplus / p0 / actual;
    let num = TWO * k - ONE + radicand.checked_sqrt().unwrap();
    num / k / TWO
}

/// Determines the spot price for tokens, excluding transaction fees, by applying the Dodo price curve equation.
///
/// ### Parameters
/// * `p0`: The reference token price. Must be a positive Decimal value or else the function will raise a panic.
/// * `target_ratio`: The target token ratio. Values should be a Decimal greater than or equal to ONE or else the
///    function will panic.
/// * `k`: The degree of concentration. Must be a Decimal within the range from MIN_K_IN to ONE permitting both
///    boundaries, otherwise, the function can raise a panic. Lower values of k represent higher concentration.
///
/// ### Function Equation
/// The spot price is calculated using the formula:
/// spot_price = (1 + k * (target_ratio ^ 2 - 1)) * p0.
///
/// ### Returns
/// This function returns a `Decimal` pointing the computed spot price of tokens.
///
/// ### Panics
/// Panics can take place if a violating value is provided as input, i.e. if input assertions fail, or if the
/// function encounters potential overflow due to unrealistic inputs.
pub fn calc_spot(p0: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(p0 > ZERO, "Invalid p0");
    assert!(target_ratio >= ONE, "Invalid target ratio");
    assert!(k >= MIN_K_IN, "Invalid k");

    let ratio2 = target_ratio * target_ratio;
    (ONE + k * (ratio2 - ONE)) * p0
}

/// Computes the reference (equilibrium) token price based on the trading curve and provided spot price. 
/// This is accomplished by rearranging the Dodo spot price curve equation to solve for the initial price.
///
/// ### Parameters
/// * `p_spot`: The spot price of the token. It is expected to be a Decimal greater than ZERO or else, the function
///    will panic.
/// * `target_ratio`: The ratio between the target amount and the actual amount of tokens. The function will panic
///    if this value is not a Decimal greater than or equal to ONE.
/// * `k`: The liquidity concentration parameter. This value should be a Decimal in the range MIN_K_IN to ONE
///    inclusively, otherwise, the function will panic. Lower values of k represent higher concentration.
///
/// ### Function Equation
/// The calculation is based on the following rearranged equation:
/// ratio2 = target_ratio * target_ratio,
/// p0 (initial price) = p_spot / (1 + k * (ratio2 - 1)).
///
/// ### Returns
/// A `Decimal` defining the computed initial price.
///
/// ### Panics
/// There can be panic situations if any of the input conditions are not met or there's a potential overflow due
/// to unrealistic inputs.
pub fn calc_p0_from_spot(p_spot: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(p_spot > ZERO, "Invalid p_spot");
    assert!(target_ratio >= ONE, "Invalid target ratio");
    assert!(k >= MIN_K_IN, "Invalid k");

    let ratio2 = target_ratio * target_ratio;
    p_spot / (ONE + k * (ratio2 - ONE))
}

/// Computes the initial token price using other parameters of the trading curve, 
/// by means of rearranging the equation derived from the integrated Dodo spot price curve.
///
/// ### Parameters
/// * `shortfall`: The shortfall of primary tokens. Must be a Decimal greater than ZERO or the function will panic.
/// * `surplus`: The amount of secondary tokens in the pool. Must be a Decimal greater than ZERO.
/// * `target_ratio`: The ratio between the target amount and the actual amount of primary tokens in the pool.
///    Must be a Decimal greater than or equal to ONE or the function will panic.
/// * `k`: The liquidity concentration parameter. This value should be a Decimal in the range MIN_K_IN to ONE
///    inclusively, otherwise, the function will panic. Lower values of k represent higher concentration.
///
/// ### Function Equation
/// The equilibrium price is calculated with this formula:
/// p0 (price at equilibrium) = surplus / shortfall / (1 + k * (target_ratio - 1)).
///
/// ### Returns
/// This function returns a `Decimal` stating the calculated initial token price.
///
/// ### Panics
/// The function will panic if any of the input assertions fail. There's a potential to overflow due for extreme
/// values of (unrealistic) input parameters.
pub fn calc_p0_from_curve(shortfall: Decimal, surplus: Decimal, target_ratio: Decimal, k: Decimal) -> Decimal {
    assert!(shortfall > ZERO, "Invalid shortfall");
    assert!(surplus > ZERO, "Invalid surplus");
    assert!(target_ratio >= ONE, "Invalid target ratio");
    assert!(k >= MIN_K_IN, "Invalid k");

    // Calculate the price at equilibrium (p0) using the given formula
    surplus / shortfall / (ONE + k * (target_ratio - ONE))
}

/// Determines the corresponding output from the given input amount by integrating along the trading curve towards
/// equilibrium. The function explicitly computes the integrated Dodo spot price curve both before and after the
/// input amount is added. The difference represents the amount of output tokens.
///
/// ### Parameters
/// * `input_amount`: Input quantity of tokens that should be greater than ZERO, otherwise, the function will panic.
/// * `target`: The desired token reserves. It must exceed the `actual` token reserves or else the function will panic.
/// * `actual`: The actual reserves of tokens. This must be a positive Decimal value, or else the function will panic.
/// * `p0`: The reference token price. This needs to be a positive Decimal value, otherwise, the function will panic.
/// * `k_in`: The liquidity concentration parameter. This value should be a Decimal in the range MIN_K_IN to ONE
///    inclusively, otherwise, the function will panic. Lower values of k represent higher concentration.
///
/// ### Function Equation
/// The output is calculated using these formulas:
/// surplus_before = (target - actual) * p0 * (1 + k_in * (target - actual) / actual),
/// surplus_after = (target - actual_after) * p0 * (1 + k_in * (target - actual_after) / actual_after),
/// The output amount is calculated as a difference between the `surplus_before` and `surplus_after`.
///
/// ### Returns
/// Returns a `Decimal` indicating the calculated output corresponding to the given input amount.
///
/// ### Panics
/// The function can panic if any of the input assertions fail. It may also overflow when extreme (unrealistic)
/// input parameters are used.
pub fn calc_incoming(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p0: Decimal,
    k_in: Decimal,
) -> Decimal {
    // Ensure the sum of the actual and input amounts does not exceed the target
    assert!(input_amount > ZERO, "Invalid input amount");
    assert!(target > actual, "Invalid target reserves");
    assert!(actual > ZERO, "Invalid actual reserves");
    assert!(p0 > ZERO, "Invalid reference price");
    assert!(k_in >= MIN_K_IN, "Invalid k_in");
    assert!(actual + input_amount <= target, "Infeasible combination");

    // Calculate the expected surplus values
    let actual_after = actual + input_amount;
    let surplus_before = (target - actual) * p0 * (ONE + k_in * (target - actual) / actual);
    let surplus_after = (target - actual_after) * p0 * (ONE + k_in * (target - actual_after) / actual_after);

    // The difference is the output amount
    surplus_before - surplus_after
}

/// This function utilizes the provided `input_amount` to calculate a corresponding output amount by integrating
/// along the trading curve away from equilibrium. It explicitly solves the quadratic formula both prior and post
/// to the `input_amount` application. The difference between these two represents the corresponding output amount.
/// 
/// Arguments:
/// * `input_amount`: The amount to be traded.
/// * `target`: The amount of primary tokens to reach equilbrium. 
/// * `actual`: The actual amount of primary tokens.
/// * `p_ref`: The reference price (price at equilibrium).
/// * `k_out`: The liquidity concentration parameter. This value should be a Decimal in the range MIN_K_IN to ONE
///    inclusively, otherwise, the function will panic. Lower values of k represent higher concentration. Note that
///    there are numeric issues close to ONE, so this value is restricted to either below 0.999, exactly ONE or
///    larger than 1.001 in the pair constructor.
/// 
/// Returns: A Decimal representing the computed output.
/// 
/// Panics: 
/// * If `input_amount` is less than or equal to zero.
/// * If `target` is less than `actual`.
/// * If `actual` is less than or equal to zero.
/// * If 'p_ref' is less than or equal to zero.
/// * If `k_out` is larger than one.
/// 
/// When `k` equals 1, a special solution is provided to avert a divide by zero situation.
/// Potential overflow might arise with wildly unrealistic prices / token amounts.
/// Note that for valid values of k_out, the square root in the calculation will always be defined.
pub fn calc_outgoing(
    input_amount: Decimal,
    target: Decimal,
    actual: Decimal,
    p_ref: Decimal,
    k_out: Decimal,
) -> Decimal {
    assert!(input_amount > ZERO, "Invalid input amount");
    assert!(target >= actual, "Invalid target reserves");
    assert!(actual > ZERO, "Invalid actual reserves");
    assert!(p_ref > ZERO, "Invalid reference price");
    assert!(k_out >= MIN_K_IN, "Invalid k_in");

    // Calculate current shortfall of tokens
    let shortfall = target - actual;

    // Calculate how many tokens should be in surplus according to the curve
    let surplus = shortfall / actual * (actual + k_out * shortfall) * p_ref;
    let scaled_new_surplus = (surplus + input_amount) / p_ref;

    // Special case for k_out equal to 1 (constant product)
    if k_out == ONE {
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
                    + (FOUR * k_out - TWO) * target * scaled_new_surplus
                    + scaled_new_surplus * scaled_new_surplus
                ).checked_sqrt().unwrap()
            )
            / TWO
            / (ONE - k_out);

        // Calculate and return the difference in shortfall
        new_shortfall - shortfall
    }
}