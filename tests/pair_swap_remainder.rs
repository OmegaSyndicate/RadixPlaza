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
        decay_factor: dec!(0),
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
fn gracefully_swaps_from_eq_when_base_is_empty() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _lp_tokens = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let (output, remainder) = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        let remainder_amount = remainder.expect("No return bucket found").amount(&mut env)?;

        assert!(output_amount == dec!(0), "Shouldn't give any output!");
        assert!(remainder_amount == dec!(1000), "All tokens should be returned");

        let (_config, state, _base_address, _quote_address, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(1), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn gracefully_swaps_from_eq_when_quote_is_empty() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _lp_tokens = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let (output, remainder) = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        let remainder_amount = remainder.expect("No return bucket found").amount(&mut env)?;

        assert!(output_amount == dec!(0), "Shouldn't give any output!");
        assert!(remainder_amount == dec!(1000), "All tokens should be returned");

        let (_config, state, _base_address, _quote_address, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(1), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn gracefully_swaps_from_quote_shortage_when_base_is_empty() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _lp_tokens = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let _swap = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        env.set_current_time(Instant::new(3391331280));
        let (output, remainder) = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        let remainder_amount = remainder.expect("No return bucket found").amount(&mut env)?;

        assert!(output_amount == dec!(1000), "Incorrect output amount");
        assert!(remainder_amount == dec!(500), "Unspent tokens should be returned");

        let (_config, state, _base_address, _quote_address, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!("0.75"), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.75"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn gracefully_swaps_from_base_shortage_when_quote_is_empty() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _lp_tokens = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let _swap = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        env.set_current_time(Instant::new(3391331280));
        let (output, remainder) = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        let output_amount = output.amount(&mut env)?;
        let remainder_amount = remainder.expect("No return bucket found").amount(&mut env)?;

        assert!(output_amount == dec!(1000), "Incorrect output amount");
        assert!(remainder_amount == dec!(500), "Unspent tokens should be returned");

        let (_config, state, _base_address, _quote_address, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1) / dec!("0.75"), "Reference price shouldn't change");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(1) / dec!("0.75"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}