use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaPair, Bucket, Bucket) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let base_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(6)
        .mint_initial_supply(500000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(8)
        .mint_initial_supply(500000, &mut env)?;

    let config = PairConfig {
        k_in: dec!("0.5"),
        k_out: dec!("1"),
        fee: dec!("0.02"),
        decay_factor: dec!("0.9512"),
    };

    let mut pair = PlazaPair::instantiate_pair(
        OwnerRole::None,
        base_bucket.resource_address(&mut env)?,
        quote_bucket.resource_address(&mut env)?,
        config,
        dec!(1),
        package,
        &mut env,
    )?;

    let _ = pair.add_liquidity(base_bucket.take(dec!(49), &mut env)?, None, &mut env)?;
    let _ = pair.add_liquidity(quote_bucket.take(dec!(49), &mut env)?, None, &mut env)?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}

// Individual tests
#[test]
fn swaps_6_to_8() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (output, _) = pair.swap(base_bucket.take(dec!(49), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == dec!(24.01), "Incorrect output amount: {}", output_amount);
        
        // Do another random swap to test divisibility works
        let _ = pair.swap(base_bucket.take(dec!(1.234567), &mut env)?, &mut env)?;
        Ok(())
    })
}

#[test]
fn swaps_8_to_6() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (output, _) = pair.swap(quote_bucket.take(dec!(49), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == dec!(24.01), "Incorrect output amount: {}", output_amount);
        
        // Do another random swap to test divisibility works
        let _ = pair.swap(quote_bucket.take(dec!(1.23456789), &mut env)?, &mut env)?;
        Ok(())
    })
}

#[test]
fn adds_6_divisibility_token() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (lp_bucket, _) = pair.add_liquidity(
            base_bucket.take_advanced(
                dec!(49),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            None,
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        let lp_expected = dec!(49);
        assert!(lp_amount == lp_expected, "Expected {} LP tokens, received {}", lp_expected, lp_amount);

        Ok(())
    })
}

#[test]
fn adds_8_divisibility_token() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (lp_bucket, _) = pair.add_liquidity(
            quote_bucket.take_advanced(
                dec!(49),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            None,
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        let lp_expected = dec!(49);
        assert!(lp_amount == lp_expected, "Expected {} LP tokens, received {}", lp_expected, lp_amount);

        Ok(())
    })
}

#[test]
fn adds_more_than_100_times_base_in_ratio() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (lp_bucket, _) = pair.add_liquidity(
            base_bucket.take_advanced(
                dec!(98_000),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            None,
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        let lp_expected = dec!(98_000);
        assert!(lp_amount == lp_expected, "Expected {} LP tokens, received {}", lp_expected, lp_amount);

        Ok(())
    })
}

#[test]
fn adds_more_than_100_times_quote_in_ratio() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (lp_bucket, _) = pair.add_liquidity(
            quote_bucket.take_advanced(
                dec!(98_000),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            None,
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        let lp_expected = dec!(98_000);
        assert!(lp_amount == lp_expected, "Expected {} LP tokens, received {}", lp_expected, lp_amount);

        Ok(())
    })
}

#[test]
fn adds_large_base_during_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _ = pair.swap(quote_bucket.take(dec!(0.0001), &mut env)?, &mut env)?;
        let (lp_bucket, _) = pair.add_liquidity(
            base_bucket.take_advanced(
                dec!(100_000),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            Some(
                quote_bucket.take_advanced(
                    dec!(100),
                    WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                    &mut env
                )?
            ),
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        let lp_expected = dec!(100_000);
        assert!(lp_amount > dec!(1.000) * lp_expected, "Expected close to {} LP tokens, received {}", lp_expected, lp_amount);
        assert!(lp_amount < dec!(1.001) * lp_expected, "Expected close to {} LP tokens, received {}", lp_expected, lp_amount);

        Ok(())
    })
}

#[test]
fn adds_small_amount_during_small_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _ = pair.swap(quote_bucket.take(dec!(0.1), &mut env)?, &mut env)?;
        let (lp_bucket, _) = pair.add_liquidity(
            base_bucket.take_advanced(
                dec!(0.01),
                WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                &mut env
            )?,
            Some(
                quote_bucket.take_advanced(
                    dec!(100),
                    WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                    &mut env
                )?
            ),
            &mut env
        )?;
        let lp_amount = lp_bucket.amount(&mut env)?;
        assert!(lp_amount == dec!(0.010019999100244978), "Unexpected LP amount: {}", lp_amount);

        Ok(())
    })
}