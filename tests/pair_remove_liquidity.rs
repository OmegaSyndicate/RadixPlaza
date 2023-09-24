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
        .mint_initial_supply(50000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(50000, &mut env)?;

    let config = PairConfig {
        k_in: dec!("0.5"),
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

    let base_lp_tokens = pair.add_liquidity(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
    let quote_lp_tokens = pair.add_liquidity(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;

    Ok(func(env, &mut pair, base_lp_tokens, quote_lp_tokens, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn removes_part_of_liquidity_when_not_in_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        _base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (primary1, secondary1) = pair.remove_liquidity(base_lp.take(dec!(50), &mut env)?, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp.take(dec!(50), &mut env)?, true, &mut env)?;

        assert!(primary1.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");

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
fn removes_all_liquidity_when_not_in_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        _base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (primary1, secondary1) = pair.remove_liquidity(base_lp, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp, true, &mut env)?;

        assert!(primary1.amount(&mut env)? == dec!(10000), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(10000), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");

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
fn removes_part_of_liquidity_when_in_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (primary1, secondary1) = pair.remove_liquidity(base_lp.take(dec!(50), &mut env)?, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp.take(dec!(50), &mut env)?, true, &mut env)?;

        assert!(primary1.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(2500), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(5000), "Incorrect secondary base amount");

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
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.25"), "Incorrect outgoing spot price deteced");
      
        Ok(())
    })
}

#[test]
fn removes_part_of_liquidity_when_in_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (primary1, secondary1) = pair.remove_liquidity(base_lp.take(dec!(50), &mut env)?, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp.take(dec!(50), &mut env)?, true, &mut env)?;

        //println!("{}", format!("{}", primary1.amount(&mut env)?));
        assert!(primary1.amount(&mut env)? == dec!(2500), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(5000), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");

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
        assert!(state.shortage == Shortage::BaseShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(4), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn removes_all_liquidity_when_in_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (primary1, secondary1) = pair.remove_liquidity(base_lp, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp, true, &mut env)?;

        assert!(primary1.amount(&mut env)? == dec!(10000), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(10000), "Incorrect secondary base amount");

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
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.25"), "Incorrect outgoing spot price deteced");
      
        Ok(())
    })
}

#[test]
fn removes_all_liquidity_when_in_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        quote_lp: Bucket,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (primary1, secondary1) = pair.remove_liquidity(base_lp, false, &mut env)?;
        let (primary2, secondary2) = pair.remove_liquidity(quote_lp, true, &mut env)?;

        //println!("{}", format!("{}", primary1.amount(&mut env)?));
        assert!(primary1.amount(&mut env)? == dec!(5000), "Incorrect primary base amount");
        assert!(secondary1.amount(&mut env)? == dec!(10000), "Incorrect secondary base amount");
        assert!(primary2.amount(&mut env)? == dec!(10000), "Incorrect primary base amount");
        assert!(secondary2.amount(&mut env)? == dec!(0), "Incorrect secondary base amount");

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
        assert!(state.shortage == Shortage::BaseShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(4), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}

#[test]
fn swaps_properly_after_emptied_in_quote_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (_primary2, _secondary2) = pair.remove_liquidity(quote_lp, true, &mut env)?;
        let (_output, _remainder) = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;

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
        assert!(state.shortage == Shortage::BaseShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!(4), "Incorrect outgoing spot price deteced");
      
        Ok(())
    })
}

#[test]
fn swaps_properly_after_emptied_in_base_shortage() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (_primary1, _secondary1) = pair.remove_liquidity(base_lp, false, &mut env)?;
        let (_output, _remainder) = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;

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
        assert!(state.shortage == Shortage::QuoteShortage, "Incorrect shortage detected");
        assert!(state.target_ratio == dec!(2), "Incorrect target ratio detected");
        assert!(state.last_out_spot == dec!("0.25"), "Incorrect outgoing spot price deteced");

        Ok(())
    })
}