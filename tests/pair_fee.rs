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
        .divisibility(18)
        .mint_initial_supply(50000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(50000, &mut env)?;

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

    let _ = pair.add_liquidity(base_bucket.take(dec!(5_000), &mut env)?, None, &mut env)?;
    let _ = pair.add_liquidity(quote_bucket.take(dec!(5_000), &mut env)?, None, &mut env)?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn applies_fee_to_swap() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (output, _) = pair.swap(base_bucket.take(dec!(5_000), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == dec!(2450), "Incorrect output amount");

        let (_config, state, _base_address, _quote_address, _bdiv, _qdiv, _base_pool, _quote_pool) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                u8,
                u8,
                ComponentAddress,
                ComponentAddress
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(5050) / dec!(2550), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.25"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}
