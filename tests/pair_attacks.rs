use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaPair, Bucket, Bucket, Bucket, Bucket) -> Result<(), RuntimeError> 
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
        k_in: dec!("0.5"),
        k_out: dec!("1"),
        fee: dec!(0),
        decay_factor: dec!(0),
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

    let (base_lp, _) = pair.add_liquidity(base_bucket.take(dec!(10_000), &mut env)?, None, &mut env)?;
    let (quote_lp, _) = pair.add_liquidity(quote_bucket.take(dec!(10_000), &mut env)?, None, &mut env)?;

    Ok(func(env, &mut pair, base_lp, quote_lp, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn swap_out_and_in() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(10), &mut env)?, &mut env)?;

        let input_amount = dec!(1000);
        let (output1, _) = pair.swap(base_bucket.take(input_amount, &mut env)?, &mut env)?;
        let (output2, _) = pair.swap(output1, &mut env)?;
        let output_amount = output2.amount(&mut env)?;

        println!("{}", format!("{} -> {}", input_amount, output_amount));
        assert!(output_amount <= input_amount, "Beep Beep Beep Beep");

        Ok(())
    })
}

#[test]
fn swap_in_and_out() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;

        let input_amount = dec!(1000);
        let (output1, _) = pair.swap(base_bucket.take(input_amount, &mut env)?, &mut env)?;
        let (output2, _) = pair.swap(output1, &mut env)?;
        let output_amount = output2.amount(&mut env)?;

        println!("{}", format!("{} -> {}", input_amount, output_amount));
        assert!(output_amount <= input_amount, "Beep Beep Beep Beep");

        Ok(())
    })
}
