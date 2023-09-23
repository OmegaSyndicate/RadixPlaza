use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;

const REF_CONFIG: PairConfig = PairConfig {
    k_in: Decimal(I192::from_digits([400000000000000022, 0, 0])),
    k_out: Decimal(I192::from_digits([1000000000000000000, 0, 0])),
    fee: Decimal(I192::from_digits([3000000000000000, 0, 0])),
    decay_factor: Decimal(I192::from_digits([951200000000000045, 0, 0])),
};

// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, PackageAddress, Bucket, Bucket, PairConfig) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let base_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;

    Ok(func(env, package, base_bucket, quote_bucket, REF_CONFIG)?)
}


// Individual tests
#[test]
fn deploys_healthy() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        config: PairConfig
    | -> Result<(), RuntimeError> {
        let pair = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        )?;

        let (config, state, base_address, quote_address, _base_pool, _quote_pool, _min_liq) = 
            env.read_component_state::<(
                PairConfig,
                PairState,
                ResourceAddress,
                ResourceAddress,
                ComponentAddress,
                ComponentAddress,
                HashMap<ComponentAddress, Vault>
            ), _>(pair).expect("Error reading state");

        assert!(config == REF_CONFIG, "Config error");
        assert!(state.p0 == dec!(1), "Price error");
        assert!(state.shortage == Shortage::Equilibrium, "Shortage error");
        assert!(state.target_ratio == dec!(1), "Target ratio error");
        assert!(state.last_out_spot == dec!(1), "Last spot price error");        
        assert!(base_address == base_bucket.resource_address(&mut env)?, "base address error");
        assert!(quote_address == quote_bucket.resource_address(&mut env)?, "quote address error");

        Ok(())
    })
}

#[test]
fn fails_on_incorrect_base_amount() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        config: PairConfig
    | {
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.0001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid base amount")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_on_incorrect_quote_amount() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        config: PairConfig
    | {
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.0001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid quote amount")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_on_low_k_in() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.k_in = dec!(0);
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid k_in value")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_if_k_out_lt_k_in() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.k_out = dec!(0);
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("k_out should be larger than k_in")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_if_k_out_too_large() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.k_out = dec!("0.9999");
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid k_out value")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_for_negative_fee() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.fee = dec!("-0.1");
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid fee level")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_for_fee_gte_one() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.fee = dec!(1);
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid fee level")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_for_negative_decay_factor() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.decay_factor = dec!("-0.5");
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid decay factor")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn fails_decay_factor_gte_one() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        quote_bucket: Bucket,
        mut config: PairConfig
    | {
        config.decay_factor = dec!(1);
        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Invalid decay factor")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn rejects_base_divisibility_unequal_18() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        _base_bucket: Bucket,
        quote_bucket: Bucket,
        config: PairConfig
    | {
        let bad_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(12)
        .mint_initial_supply(20000, &mut env)?;

        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            bad_bucket.take(dec!("0.000001"), &mut env)?,
            quote_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Bad base divisibility")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[test]
fn rejects_quote_divisibility_unequal_18() -> Result<(), RuntimeError> {
    let _ = publish_and_setup(|
        mut env, 
        package: PackageAddress,
        base_bucket: Bucket,
        _quote_bucket: Bucket,
        config: PairConfig
    | {
        let bad_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(12)
        .mint_initial_supply(20000, &mut env)?;

        let result = PlazaPair::instantiate_pair(
            OwnerRole::None,
            base_bucket.take(dec!("0.000001"), &mut env)?,
            bad_bucket.take(dec!("0.000001"), &mut env)?,
            config,
            dec!(1),
            package,
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("Bad quote divisibility")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })?;
    Ok(())
}