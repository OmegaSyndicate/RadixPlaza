use defiplaza::pair::test_bindings::*;
use defiplaza::types::PairConfig;
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
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;

    let config = PairConfig {
        k_in: dec!("0.4"),
        k_out: dec!("1"),
        fee: dec!(0),
        decay_factor: dec!("0.9512"),
    };

    let mut pair = PlazaPair::instantiate_pair(
        OwnerRole::None,
        base_bucket.take(dec!("0.000001"), &mut env)?,
        quote_bucket.take(dec!("0.000001"), &mut env)?,
        config,
        dec!(1),
        package,
        &mut env,
    )?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn initial_add_gives_correct_lp() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let add_base = pair.add_liquidity(base_bucket.take(dec!(10000), &mut env)?, false, &mut env)?;
        let add_quote = pair.add_liquidity(quote_bucket.take(dec!(10000), &mut env)?, true, &mut env)?;
        assert!(add_base.amount(&mut env)? == dec!(100), "Unexpected LP amount");
        assert!(add_quote.amount(&mut env)? == dec!(100), "Unexpected LP amount");
        Ok(())
    })
}

#[test]
fn second_add_goes_in_ratio() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _ = pair.add_liquidity(base_bucket.take(dec!(10000), &mut env)?, false, &mut env)?;
        let _ = pair.add_liquidity(quote_bucket.take(dec!(10000), &mut env)?, true, &mut env)?;
        let add_base = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, false, &mut env)?;
        let add_quote = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, true, &mut env)?;
        let error = ApplicationError::PanicMessage("bluh".to_string());
        assert!(add_base.amount(&mut env)?
            .checked_round(
                8,
                RoundingMode::ToNearestMidpointAwayFromZero
            ).ok_or(error.clone())? == dec!(10), "Unexpected LP amount");
        assert!(add_quote.amount(&mut env)?
            .checked_round(
                8,
                RoundingMode::ToNearestMidpointAwayFromZero
            ).ok_or(error.clone())? == dec!(10), "Unexpected LP amount");
        Ok(())
    })
}