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
        .mint_initial_supply(20000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;

    let config = PairConfig {
        k_in: dec!("0.5"),
        k_out: dec!("1"),
        fee: dec!(0),
        decay_factor: dec!("0.9512"),
    };

    let mut pair = PlazaPair::instantiate_pair(
        OwnerRole::None,
        base_bucket.take(dec!("0.0001"), &mut env)?,
        quote_bucket.take(dec!("0.0001"), &mut env)?,
        config,
        dec!(1),
        package,
        &mut env,
    )?;

    let _ = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, None, &mut env)?;
    let _ = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, None, &mut env)?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn swaps_base_to_quote() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;

        let (_config, state, _base_address, _quote_address, _bdiv, _qdiv, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                u8,
                u8,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.25"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn swaps_quote_to_base() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;

        let (_config, state, _base_address, _quote_address, _bdiv, _qdiv, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                u8,
                u8,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::BaseShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(4), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn single_swaps_base_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap, _) = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;
        assert!(swap.amount(&mut env)? == dec!(750), "Incorrect return amount");
        Ok(())
    })
}

#[test]
fn double_swaps_base_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap1, _) = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let (swap2, _) = pair.swap(base_bucket.take(dec!(2000), &mut env)?, &mut env)?;
        let total_amount = swap1.amount(&mut env)? + swap2.amount(&mut env)?;
        assert!(total_amount == dec!(750), "Incorrect return amount");
        Ok(())
    })
}

#[test]
fn single_swaps_quote_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap, _) = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;
        assert!(swap.amount(&mut env)? == dec!(750), "Incorrect return amount");
        Ok(())
    })
}

#[test]
fn double_swaps_quote_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap1, _) = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let (swap2, _) = pair.swap(quote_bucket.take(dec!(2000), &mut env)?, &mut env)?;
        let total_amount = swap1.amount(&mut env)? + swap2.amount(&mut env)?;
        assert!(total_amount == dec!(750), "Incorrect return amount");
        Ok(())
    })
}