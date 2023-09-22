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

    let _lp_tokens = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, false, &mut env)?;
    let lp_tokens = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, true, &mut env)?;
    //println!("LP amount: {}", lp_tokens.amount(&mut env)?);

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn swaps() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(1), &mut env)?, &mut env)?;
        Ok(())
    })?;
    Ok(())
}