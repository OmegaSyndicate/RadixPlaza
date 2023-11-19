use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;

// This test module cherry-picks numbers for k_in, k_out and the initial shortage such that the
// math works out nicely and we get round numbers everywhere.

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
fn setup_correct_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;

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
        assert!(state.target_ratio == dec!(4), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.0625"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn setup_correct_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;

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
        assert!(state.target_ratio == dec!(4), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(16), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn gives_correct_spot_price_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;

        let input_quote_amount = dec!("0.000001");
        let (output, _) = pair.swap(quote_bucket.take(input_quote_amount, &mut env)?, &mut env)?;
        let output_base_amount = output.amount(&mut env)?.checked_round(
            12,
            RoundingMode::ToNearestMidpointAwayFromZero,
        ).unwrap();

        assert!(output_base_amount == 13 * input_quote_amount, "Incorrect spot price");

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
        assert!(state.target_ratio < dec!(4), "Too large target ratio detected");
        assert!(state.target_ratio * (dec!(250) + input_quote_amount) > dec!(1000), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.0625"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn gives_correct_spot_price_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;

        let input_quote_amount = dec!("0.000001");
        let (output, _) = pair.swap(base_bucket.take(input_quote_amount, &mut env)?, &mut env)?;
        let output_base_amount = output.amount(&mut env)?.checked_round(
            12,
            RoundingMode::ToNearestMidpointAwayFromZero,
        ).unwrap();

        assert!(output_base_amount == 13 * input_quote_amount, "Incorrect spot price");

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
        assert!(state.target_ratio < dec!(4), "Too large target ratio detected");
        assert!(state.target_ratio * (dec!(250) + input_quote_amount) > dec!(1000), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(16), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn trades_correct_amount_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;        
        let (output, _) = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
 
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

        assert!(output.amount(&mut env)? == dec!(3000), "Incorrect trade sizing");
        assert!(state.p0 == dec!(1), "Reference price shouldn't have changed");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(1), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn trades_correct_amount_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;        
        let (output, _) = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;

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

        assert!(output.amount(&mut env)? == dec!(3000), "Incorrect trade sizing");
        assert!(state.p0 == dec!(1), "Reference price shouldn't have changed");
        assert!(state.shortage == Shortage::Equilibrium, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(1), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(1), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn trades_correct_amount_accross_eq_from_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;        
        let (output, _) = pair.swap(quote_bucket.take(dec!(4000), &mut env)?, &mut env)?;
        
        assert!(output.amount(&mut env)? == dec!(3750), "Incorrect trade sizing");
 
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

        assert!(state.p0 == dec!(1), "Reference price shouldn't have changed");
        assert!(state.shortage == Shortage::BaseShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(4), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(16), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn trades_correct_amount_accross_eq_from_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _outgoing = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;        
        let (output, _) = pair.swap(base_bucket.take(dec!(4000), &mut env)?, &mut env)?;
        
        assert!(output.amount(&mut env)? == dec!(3750), "Incorrect trade sizing");
 
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

        assert!(state.p0 == dec!(1), "Reference price shouldn't have changed");
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(4), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.0625"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}