use scrypto::prelude::*;
use crate::events::*;
use crate::types::*;

#[blueprint]
#[events(SwapEvent, AddLiquidityEvent, RemoveLiquidityEvent)]
mod plazapair {
    // PlazaPair struct represents a liquidity pair in the trading platform
    struct PlazaPair {
        config: PairConfig,                 // Pool configuration
        state: PairState,                   // Pool parameters
        base_vault: Vault,                  // Holds the base tokens
        quote_vault: Vault,                 // Holds the quote tokens
        base_lp: ResourceManager,           // Resource address of base LP tokens
        quote_lp: ResourceManager,          // Resource address of quote LP tokens
    }

    impl PlazaPair {
        pub fn instantiate_pair(
            base_token: ResourceAddress,
            quote_token: ResourceAddress,
            config: PairConfig,
            initial_price: Decimal,
        ) -> Global<PlazaPair> {
            assert!(config.k_in >= dec!("0.001"), "Invalid k_in value");
            assert!(config.k_out > config.k_in, "k_out should be larger than k_in");
            assert!(config.k_out == dec!(1) || config.k_out < dec!("0.999"), "Invalid k_out value");
            assert!(config.fee >= dec!(0) && config.fee < dec!(1), "Invalid fee level");

            let base_manager = ResourceManager::from(base_token);
            let quote_manager = ResourceManager::from(quote_token);
            assert!(base_manager.resource_type().is_fungible(), "Non-fungible base token detected");
            assert!(quote_manager.resource_type().is_fungible(), "Non-fungible quote token detected");

            // Reserve address for Actor Virtual Badge
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(PlazaPair::blueprint_id());

            // Create LP tokens for the base token providers
            let base_lp: ResourceManager = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata! {
                    init {
                        "name" => "PlazaPair Base LP", locked;
                        "symbol" => "BASELP", locked;
                    }
                })
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_caller(component_address))); 
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_caller(component_address)));
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            // Create LP tokens for the quote token providers
            let quote_lp: ResourceManager = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata! {
                    init {
                        "name" => "PlazaPair Quote LP", locked;
                        "symbol" => "QUOTELP", locked;
                    }
                })
                .mint_roles(mint_roles! {
                    minter => rule!(require(global_caller(component_address))); 
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_caller(component_address)));
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            // Instantiate a PlazaPair component
            let now = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            Self {
                config: config,
                state: PairState {
                    p0: initial_price,
                    base_target: dec!(0),
                    quote_target: dec!(0),
                    shortage: Shortage::Equilibrium,
                    last_trade: now,
                    last_outgoing: now,
                    last_spot: initial_price,
                },
                base_vault: Vault::new(base_token),
                quote_vault: Vault::new(quote_token),
                base_lp: base_lp,
                quote_lp: quote_lp,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::None)
            .with_address(address_reservation)
            .globalize()
        }

        // Add liquidity to the pool in return for LP tokens
        pub fn add_liquidity(&mut self, input_bucket: Bucket) -> Bucket {
            let token_amount = input_bucket.amount();
            assert!(token_amount > dec!(0), "Empty bucket provided");
        
            // Determine if the bucket is for the quote or the base pool
            let is_quote = self.quote_vault.resource_address() == input_bucket.resource_address();
            if !is_quote {
                assert!(
                    input_bucket.resource_address() == self.base_vault.resource_address(),
                    "Invalid bucket"
                );
            }
        
            // Based on the bucket type, choose the correct vault, target and resource address
            let (vault, target_amount, lp_manager) = if is_quote {
                (
                    &mut self.quote_vault,
                    &mut self.state.quote_target,
                    self.base_lp,
                )
            } else {
                (
                    &mut self.base_vault,
                    &mut self.state.base_target,
                    self.quote_lp,
                )
            };

            // Calculate new LP tokens HARDCODED
            let lp_amount = dec!(1);

            // Emit add liquidity event
            Runtime::emit_event(AddLiquidityEvent{is_quote, token_amount, lp_amount});

            // Take in liquidity, update target and mint the new LP tokens
            vault.put(input_bucket);
            *target_amount = *target_amount + token_amount;
            lp_manager.mint(lp_amount)
        }

        // Exchange LP tokens for the underlying liquidity held in the pair
        // TODO -- ENSURE HEALTH WITH ZERO LIQ
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            // Ensure the bucket isn't empty
            let lp_amount = lp_tokens.amount();
            assert!(lp_amount > dec!(0), "Empty bucket provided");
            assert!(
                lp_tokens.resource_address() == self.quote_lp.address() ||
                    lp_tokens.resource_address() == self.base_lp.address(), 
                "Invalid LP tokens"
            );

            // Determine which vault and target values should be used
            let is_quote = lp_tokens.resource_address() == self.quote_lp.address();
            let (main_vault, other_vault, main_target, other_target, is_shortage, lp_manager) = if is_quote {
                (
                    &mut self.quote_vault,
                    &mut self.base_vault,
                    &mut self.state.quote_target,
                    self.state.base_target,
                    self.state.shortage == Shortage::QuoteShortage,
                    self.quote_lp,
                )
            } else {
                (
                    &mut self.base_vault,
                    &mut self.quote_vault,
                    &mut self.state.base_target,
                    self.state.quote_target,
                    self.state.shortage == Shortage::BaseShortage,
                    self.base_lp,
                )
            };

            // Calculate fraction of liquidity being withdrawn
            let lp_outstanding = lp_manager.total_supply().unwrap();
            let fraction = lp_amount / lp_outstanding;

            // Calculate how many tokens are represented by the withdrawn LP tokens
            let (main_amount, other_amount) = if is_shortage {                
                let surplus = other_vault.amount() - other_target;
                (
                    fraction * main_vault.amount(),
                    fraction * surplus,
                )
            } else {
                (
                    fraction * *main_target,
                    dec!(0),
                )
            };

            // Burn the LP tokens and update the target value
            lp_tokens.burn();
            *main_target -= fraction * *main_target;

            // Emit RemoveLiquidityEvent
            Runtime::emit_event(RemoveLiquidityEvent{is_quote, main_amount, other_amount, lp_amount});

            // Take liquidity from the vault and return to the caller
            (main_vault.take(main_amount), other_vault.take(other_amount))
        }

        /// Swap a bucket of tokens along the AMM curve.
        pub fn swap(&mut self, input_tokens: Bucket) -> Bucket {
            // Ensure the input bucket isn't empty
            assert!(input_tokens.amount() > dec!(0), "Empty input bucket");

            // Calculate the amount of output tokens and pair impact variables.
            let input_amount = input_tokens.amount();
            let is_quote = input_tokens.resource_address() == self.quote_vault.resource_address();
            let (output_amount, fee, mut new_state) = self.quote(input_amount, is_quote);

            // Log trade event
            let (base_amount, quote_amount) = match is_quote{
                true => (-output_amount, input_amount),
                false => (input_amount, -output_amount),
            };
            Runtime::emit_event(SwapEvent{base_amount, quote_amount});

            // Update the target values and select the input and output vaults based on input_tokens type.
            let (input_vault, output_vault) = if is_quote {
                new_state.base_target += fee;
                (&mut self.quote_vault, &mut self.base_vault)
            } else {
                new_state.quote_target += fee;
                (&mut self.base_vault, &mut self.quote_vault)
            };

            // Adjust pair state variables.
            self.state = new_state;

            // Transfer the tokens.
            input_vault.put(input_tokens);
            output_vault.take(output_amount)
        }

        // Getter function to identify related LP tokens
        pub fn get_lp_tokens(&self) -> (ResourceAddress, ResourceAddress) {
            (self.base_lp.address(), self.quote_lp.address())
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(&self, input_amount: Decimal, input_is_quote: bool) -> (Decimal, Decimal, PairState) {
            let new_state = self.state;

            let output_amount = dec!(1);
            let fee = dec!(0);

            (output_amount - fee, fee, new_state)
        }
    }
}
