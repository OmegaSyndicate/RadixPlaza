use scrypto::prelude::*;
use crate::events::*;
use crate::helpers::*;
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
            initial_price: Decimal,
        ) -> Global<PlazaPair> {
            let config = PairConfig {
                k_in: dec!("0.4"),
                k_out: dec!("1"),
                fee: dec!("0.003"),
            };
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
            let (vault, other_vault, target_amount, other_target, lp_manager) = if is_quote {
                (
                    &mut self.quote_vault,
                    &self.base_vault,
                    &mut self.state.quote_target,
                    self.state.base_target,
                    self.quote_lp
                )
            } else {
                (
                    &mut self.base_vault,
                    &self.quote_vault,
                    &mut self.state.base_target,
                    self.state.quote_target,
                    self.base_lp
                )
            };
            let (actual_amount, other_amount) = ((*vault).amount(), (*other_vault).amount());
            
            // Check amount of outstanding LP tokens
            let lp_outstanding = lp_manager.total_supply().unwrap();
        
            // Calculate the new LP amount
            let (new_target, lp_amount) = match (lp_outstanding == dec!(0), actual_amount < *target_amount) {
                (true,_) => (token_amount, token_amount),
                // If the token being added is in shortage, we need to issue LP tokens in ratio with
                // the increase in target amount that keeps the trading curve anchored to the right
                // value of p0. Note that we assume the low-pass filter to be at steady state as that
                // is the conservative approach when issuing new LP tokens.
               (_,true) => {
                    let k = self.config.k_in;
                    let surplus = other_amount - other_target;
                    let p0 = calc_p0_from_surplus(surplus, *target_amount, actual_amount, k);
                    let current_target = calc_target(p0, actual_amount, surplus, k);
                    let new_target = calc_target(p0, actual_amount + token_amount, surplus, k);
                    (new_target, (new_target - current_target) / current_target * lp_outstanding)
                }
                // If this token is not in shortage we can simply issue in ratio with existing amount
                (_,_) => {
                    (*target_amount + token_amount, token_amount / *target_amount * lp_outstanding)
                }
            };
        
            // Emit add liquidity event
            Runtime::emit_event(AddLiquidityEvent{is_quote, token_amount, lp_amount});

            // Take in liquidity, update target and mint the new LP tokens
            vault.put(input_bucket);
            *target_amount = new_target;
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

            let (token_in, token_out) = if is_quote {
                ("quote", "base")
            } else {
                ("base", "quote")
            };
            info!("  SWAP: {} {} for {} {}.", input_amount, token_in, output_amount, token_out);

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
            debug!("  p0: {} -- state {}", new_state.p0, new_state.shortage);

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
            let mut new_state = self.state;

            // Compute time since previous trade
            let t = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            let delta_t = if t > self.state.last_trade { 
                t - self.state.last_trade
            } else { 
                0
            };
            new_state.last_trade = t;
            debug!("  delta_t: {} -- last_trade {}", delta_t, self.state.last_trade);

            // Compute decay factor with a 15 minute time constant
            let factor = Decimal::checked_powi(&dec!("0.9355"), delta_t / 60).unwrap();
            debug!("  old-new p0 skew factor {}", factor);

            // Collect current actual balances
            let (base_actual, quote_actual) = (self.base_vault.amount(), self.quote_vault.amount());

            // Set the input and output vaults and targets based on input_tokens type
            let (input_actual, mut output_actual, input_target, mut output_target) =
                if input_is_quote {
                    (
                        quote_actual,
                        base_actual,
                        self.state.quote_target,
                        self.state.base_target,
                    )
                } else {
                    (
                        base_actual,
                        quote_actual,
                        self.state.base_target,
                        self.state.quote_target,
                    )
                };            

            // Calculate the base[quote] price in equilibrium
            let new_p0 = match self.state.shortage {
                Shortage::BaseShortage => {
                    let quote_surplus = quote_actual - self.state.quote_target;
                    calc_p0_from_surplus(quote_surplus, self.state.base_target, base_actual, self.config.k_in)
                }
                Shortage::Equilibrium => self.state.p0,
                Shortage::QuoteShortage => {
                    let base_surplus = base_actual - self.state.base_target;
                    dec!(1) / calc_p0_from_surplus(base_surplus, self.state.quote_target, quote_actual, self.config.k_in)
                }
            };
            let p0 = factor * self.state.p0 + (dec!(1) - factor) * new_p0;
            new_state.p0 = p0;
            debug!("  old_p0: {}, new_p0: {}, skewed_p0: {}", self.state.p0, new_p0, p0);


            // Set reference price to B[Q] or Q[B] depending on liquidity
            let (mut p_ref, mut last_spot) = 
                match self.state.shortage {
                    Shortage::BaseShortage => { (p0, self.state.last_spot) },
                    Shortage::Equilibrium => {
                        if input_is_quote {
                            (p0, self.state.last_spot)
                        } else {
                            (dec!(1) / p0, dec!(1) / self.state.last_spot)
                        }
                    },
                    Shortage::QuoteShortage => { (dec!(1) / p0, dec!(1) / self.state.last_spot) },
                };
            debug!("  p_ref used: {}", p_ref);

            // Determine which state represents input shortage based on input type
            let in_input_shortage = match input_is_quote {
                true => Shortage::QuoteShortage,
                false => Shortage::BaseShortage
            };
            debug!("  Input shortage indicated as {}", in_input_shortage);

            // Define running counters
            let mut amount_to_trade = input_amount;
            let mut output_amount = dec!(0);

            // Handle the incoming case (trading towards equilibrium). We project the current reserves on the
            // incoming curve by calculating a adjusted target value to reach equilibrium and spend all surplus
            // counter tokens. If we go past equilibrium, we update state accordingly. Note that we ignore the
            // stored target value to elegantly deal with the excess tokens from earlier from the sparser
            // liquidity on the curve trading away from equilibrium.
            if new_state.shortage == in_input_shortage {
                let surplus = output_actual - output_target;
                let adjusted_target = calc_target(p_ref, input_actual, surplus, self.config.k_in);
                let shortfall = adjusted_target - input_actual;
                debug!("  Input shortage of {} detected.", shortfall);
                debug!("  Skew factor: {}, last_spot: {}, p_ref: {}", factor, last_spot, p_ref);                
                debug!("  Amount: {}, adj_target: {}, actual: {}", input_amount, adjusted_target, input_actual);

                // If we add more than required to reach equilibrium, we reset to equilibrium and continue the
                // trade on the outgoing curve below.
                if amount_to_trade >= shortfall {
                    debug!("  Trading to/past equilibrium. Leg input: {} -- output: {}", shortfall, surplus);
                    output_amount = surplus;
                    amount_to_trade -= shortfall;
                    output_target = output_actual - output_amount;
                    output_actual = output_target;
                    p_ref = dec!(1) / p_ref;    
                    last_spot = p_ref;
                    
                    // Update state variables to match equilibrium values
                    new_state.set_input_target(adjusted_target, input_is_quote);
                    new_state.set_output_target(output_target, input_is_quote);
                    new_state.last_spot = p0;
                    new_state.shortage = Shortage::Equilibrium;
                } else {
                    // If we stay in the same shortage situation, we calculate according to the incoming curve.
                    output_amount = calc_incoming(
                        input_amount,
                        adjusted_target,
                        input_actual,
                        p_ref,
                        self.config.k_in,
                    );

                    // Prevent actual value from being more than the stored target value
                    let new_input_actual = input_actual + amount_to_trade;
                    if new_input_actual > input_target {
                        new_state.set_input_target(new_input_actual, input_is_quote);
                    }
                }
            }

            // Handle the trading away from equilbrium case
            if new_state.shortage != in_input_shortage && amount_to_trade > dec!(0) {
                debug!("  Trade on outgoing curve");
                // Calibrate outgoing price curve to filtered spot price.
                let incoming_spot = calc_spot(p_ref, output_target, output_actual, self.config.k_in);
                let outgoing_spot = factor * last_spot + (dec!(1) - factor) * incoming_spot;
                let virtual_p_ref = calc_p0_from_spot(outgoing_spot, output_target, output_actual, self.config.k_out);

                debug!("  Skew factor: {}, last_spot: {} p_ref: {}", factor, last_spot, p_ref);                
                debug!("  Amount: {}, target: {}, actual {}, virtual_p_ref {}", amount_to_trade, output_target, output_actual, virtual_p_ref);
                // Calculate output amount based on outgoing curve
                let outgoing_output = calc_outgoing(
                    amount_to_trade,
                    output_target,
                    output_actual,
                    virtual_p_ref,
                    self.config.k_out,
                );
                output_amount += outgoing_output;

                // Select what the new exchange state should be
                new_state.shortage = match in_input_shortage {
                    Shortage::QuoteShortage => Shortage::BaseShortage,
                    Shortage::BaseShortage => Shortage::QuoteShortage,
                    Shortage::Equilibrium => {
                        if input_is_quote {
                            Shortage::BaseShortage
                        } else {
                            Shortage::QuoteShortage
                        }
                    }
                };

                // Store previous outgoing time stamp
                new_state.last_outgoing = t;

                // Calculate and store previous outgoing spot price
                let new_actual = output_actual - outgoing_output;
                new_state.last_spot = match new_state.shortage {
                    Shortage::BaseShortage => calc_spot(virtual_p_ref, output_target, new_actual, self.config.k_out),
                    Shortage::Equilibrium => p0,
                    Shortage::QuoteShortage => dec!(1) / calc_spot(virtual_p_ref, output_target, new_actual, self.config.k_out), 
                };
            }

            debug!("  Last spot: {}", new_state.last_spot);

            // Apply trading fee
            let fee = self.config.fee * output_amount;

            (output_amount - fee, fee, new_state)
        }
    }
}
