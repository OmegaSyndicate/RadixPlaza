use defiplaza::dex::test_bindings::*;
use defiplaza::types::PairConfig;
use scrypto::*;
use scrypto_test::prelude::*;
use scrypto::prelude::ToRoleEntry;
use crate::node_modules::auth::RoleDefinition;
//use scrypto::prelude::Url;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaDex, Bucket, Bucket, Bucket) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let a_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;
    let b_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;
    let dfp2_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;

    let admin_badge = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1, &mut env)?;
    let admin_address = admin_badge.resource_address(&mut env)?;
    let dfp2_address = dfp2_bucket.resource_address(&mut env)?; 

    let mut dex = PlazaDex::instantiate_dex(
        dfp2_address,
        admin_address,
        package,
        &mut env
    )?;

    let config = PairConfig {
        k_in: dec!("0.4"),
        k_out: dec!("1"),
        fee: dec!("0"),
        decay_factor: dec!("0.9512"),
    };
    dex.create_pair( 
        a_bucket.take(dec!(1000), &mut env)?,
        dfp2_bucket.take(dec!(1000), &mut env)?,
        config,
        dec!(1),
        &mut env,
    )?;
    dex.create_pair( 
        b_bucket.take(dec!(1000), &mut env)?,
        dfp2_bucket.take(dec!(1000), &mut env)?,
        config,
        dec!(1),
        &mut env,
    )?;

    Ok(func(env, &mut dex, a_bucket, b_bucket, dfp2_bucket)?)
}

// Individual tests
#[test]
fn deploys() -> Result<(), RuntimeError> {
    publish_and_setup(|mut _env, &mut _dex, _a_bucket, _b_bucket, _dfp2_bucket| -> Result<(), RuntimeError> {
        Ok(())
    })
}

#[test]
fn rejects_outright_recallable_tokens() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, _a_bucket, _b_bucket, dfp2_bucket| -> Result<(), RuntimeError> {
        let config = PairConfig {
            k_in: dec!("1"),
            k_out: dec!("2"),
            fee: dec!("0.005"),
            decay_factor: dec!("0.998"),
        };

        let c_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
            .divisibility(18)
            .recall_roles(recall_roles! {
                recaller => rule!(require(dfp2_bucket.resource_address(&mut env)?));
                recaller_updater => rule!(deny_all);
            })
            .mint_initial_supply(10000, &mut env)?;

        let result = dex.create_pair( 
            c_bucket.take(dec!(1000), &mut env)?,
            dfp2_bucket.take(dec!(1000), &mut env)?,
            config,
            dec!(1),
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("assertion failed:")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })
}

#[test]
fn rejects_potentially_recallable_tokens() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, _a_bucket, _b_bucket, dfp2_bucket| -> Result<(), RuntimeError> {
        let config = PairConfig {
            k_in: dec!("1"),
            k_out: dec!("2"),
            fee: dec!("0.005"),
            decay_factor: dec!("0.998"),
        };

        let c_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
            .divisibility(18)
            .recall_roles(recall_roles! {
                recaller => rule!(deny_all);
                recaller_updater => rule!(require(dfp2_bucket.resource_address(&mut env)?));
            })
            .mint_initial_supply(10000, &mut env)?;

        let result = dex.create_pair( 
            c_bucket.take(dec!(1000), &mut env)?,
            dfp2_bucket.take(dec!(1000), &mut env)?,
            config,
            dec!(1),
            &mut env,
        );
        match result {
            Ok(_) => panic!("Should've thrown an error!"),
            Err(e) => {
                assert!(
                    matches!(e, RuntimeError::ApplicationError(ApplicationError::PanicMessage(ref pm)) 
                        if pm.starts_with("assertion failed:")),
                    "Actual error thrown: {:?}", e);
                Ok(())
            }
        }
    })
}