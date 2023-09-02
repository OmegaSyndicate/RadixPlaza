use scrypto::prelude::*;
use crate::plaza_events::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum Shortage {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct PairState {
    p0: Decimal,                // Equilibrium price
    base_target: Decimal,       // Target amount of base tokens
    quote_target: Decimal,      // Target amount of quote tokens
    shortage: Shortage,         // Current state of the pair
    last_trade: i64,            // Timestamp of last trade
    last_outgoing: i64,         // Timestamp of last outgoing trade
    last_spot: Decimal,     // Last outgoing spot price
}

impl PairState {
    pub fn set_output_target(&mut self, output_target: Decimal, input_is_quote: bool) {
        if input_is_quote {
            self.base_target = output_target;
        } else {
            self.quote_target = output_target;
        }
    }

    pub fn set_input_target(&mut self, input_target: Decimal, input_is_quote: bool) {
        if input_is_quote {
            self.quote_target = input_target;
        } else {
            self.base_target = input_target;
        }
    }
}

#[derive(ScryptoSbor)]
pub struct PairConfig {
    k_in: Decimal,              // Ingress price curve exponent
    k_out: Decimal,             // Egress price curve exponent
    fee: Decimal,               // Trading fee
}

impl fmt::Display for Shortage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Shortage::BaseShortage => write!(f, "BaseShortage"),
            Shortage::Equilibrium => write!(f, "Equilibrium"),
            Shortage::QuoteShortage => write!(f, "QuoteShortage"),
        }
    }
}

#[blueprint]
#[events(SwapEvent)]
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
            let base_manager = ResourceManager::from(base_token);
            let quote_manager = ResourceManager::from(quote_token);
            assert!(base_manager.resource_type().is_fungible(), "non-fungible base token detected");
            assert!(quote_manager.resource_type().is_fungible(), "non-fungible quote token detected");

            // Reserve address for Actor Virtual Badge
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Runtime::blueprint_id());

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
                config: PairConfig {
                    k_in: dec!("0.4"),
                    k_out: dec!("1"),
                    fee: dec!("0.003"),
                },
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
            assert!(input_bucket.amount() > dec!(0), "Empty bucket provided");
        
            // Determine if the bucket is for the quote or the base pool
            let is_quote = self.quote_vault.resource_address() == input_bucket.resource_address();
        
            // Based on the bucket type, choose the correct vault, target and resource address
            let (vault, target_value, lp_manager) = if is_quote {
                (&mut self.quote_vault, &mut self.state.quote_target, self.quote_lp)
            } else {
                (&mut self.base_vault, &mut self.state.base_target, self.base_lp)
            };
        
            // Check amount of outstanding LP tokens
            let lp_outstanding = lp_manager.total_supply().unwrap();
        
            // Calculate the new LP amount
            let new_lp_value = if lp_outstanding == dec!(0) { 
                input_bucket.amount()
            } else {
                input_bucket.amount() / *target_value * lp_outstanding
            };
        
            // Update target field and take in liquidity
            *target_value += input_bucket.amount();
            vault.put(input_bucket);

            // Mint the new LP tokens
            lp_manager.mint(new_lp_value)
        }

        // Exchange LP tokens for the underlying liquidity held in the pair
        // TODO -- ENSURE HEALTH WITH ZERO LIQ
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            // Ensure the bucket isn't empty
            assert!(lp_tokens.amount() > dec!(0), "Empty bucket provided");
            assert!(
                lp_tokens.resource_address() == self.quote_lp.address() ||
                    lp_tokens.resource_address() == self.base_lp.address(), 
                "Invalid LP tokens"
            );

            // Determine which vault and target values should be used
            let is_quote = lp_tokens.resource_address() == self.quote_lp.address();
            let (this_vault, other_vault, this_target, other_target, is_shortage, lp_manager) = if is_quote {
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
            let fraction = lp_tokens.amount() / lp_outstanding;

            // Calculate how many tokens are represented by the withdrawn LP tokens
            let (this_amount, other_amount) = if is_shortage {                
                let surplus = other_vault.amount() - other_target;
                (
                    fraction * this_vault.amount(),
                    fraction * surplus,
                )
            } else {
                (
                    fraction * *this_target,
                    dec!(0),
                )
            };

            // Burn the LP tokens and update the target value
            lp_tokens.burn();
            *this_target -= fraction * *this_target;

            // Take liquidity from the vault and return to the caller
            (this_vault.take(this_amount), other_vault.take(other_amount))
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
            let factor = Decimal::powi(&dec!("0.9355"), delta_t / 60);
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
                    self.calc_p0_from_surplus(quote_surplus, self.config.k_in, self.state.base_target, base_actual)
                }
                Shortage::Equilibrium => self.state.p0,
                Shortage::QuoteShortage => {
                    let base_surplus = base_actual - self.state.base_target;
                    dec!(1) / self.calc_p0_from_surplus(base_surplus, self.config.k_in, self.state.quote_target, quote_actual)
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

            // Handle the incoming case (trading towards equilibrium)
            if new_state.shortage == in_input_shortage {
                let surplus = output_actual - output_target;
                let adjusted_target = self.calc_target(p_ref, self.config.k_in, input_actual, surplus);
                let shortfall = adjusted_target - input_actual;
                let leg_amount = std::cmp::min(amount_to_trade, shortfall);
                debug!("  Input shortage of {} detected.", shortfall);
                debug!("  Skew factor: {}, last_spot: {}, p_ref: {}", factor, last_spot, p_ref);                
                debug!("  Amount: {}, target: {}, adj_target {}, actual {}", leg_amount, input_target, adjusted_target, input_actual);
                output_amount = self.calc_incoming(
                    leg_amount,
                    adjusted_target,
                    input_actual,
                    p_ref,
                );

                new_state.set_input_target(adjusted_target, input_is_quote);
                if amount_to_trade >= shortfall {
                    debug!("  Trading to/past equilibrium. First leg {}", output_amount);
                    amount_to_trade -= shortfall;
                    output_target = output_actual - output_amount;
                    output_actual = output_target;
                    p_ref = dec!(1) / p_ref;    
                    last_spot = p_ref;
                    
                    // Update state variables
                    new_state.set_output_target(output_target, input_is_quote);
                    new_state.last_spot = p0;
                    new_state.shortage = Shortage::Equilibrium;
                } else {
                    new_state.last_spot = match new_state.shortage {
                        Shortage::BaseShortage => self.calc_spot(p_ref, self.config.k_in, adjusted_target, input_actual + leg_amount),
                        Shortage::Equilibrium => p0,
                        Shortage::QuoteShortage => { dec!(1) / self.calc_spot(p_ref, self.config.k_in, adjusted_target, input_actual + leg_amount) }, 
                    };
                }
            }

            // Handle the trading away from equilbrium case
            if new_state.shortage != in_input_shortage && amount_to_trade > dec!(0) {
                debug!("  Trade on outgoing curve");
                // Calibrate outgoing price curve to incoming at spot price.
                let virtual_p_ref = self.calc_p0_from_spot(last_spot, self.config.k_out, output_target, output_actual);

                debug!("  Skew factor: {}, last_spot: {}, p_ref: {}", factor, last_spot, p_ref);                
                debug!("  Amount: {}, target: {}, actual {}, virtual_p_ref {}", amount_to_trade, output_target, output_actual, virtual_p_ref);
                // Calculate output amount based on outgoing curve
                let outgoing_output = self.calc_outgoing(
                    amount_to_trade,
                    output_target,
                    output_actual,
                    virtual_p_ref,
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
                    Shortage::BaseShortage => self.calc_spot(virtual_p_ref, self.config.k_out, output_target, new_actual),
                    Shortage::Equilibrium => p0,
                    Shortage::QuoteShortage => dec!(1) / self.calc_spot(virtual_p_ref, self.config.k_out, output_target, new_actual), 
                };
            }

            debug!("  Last spot: {}", new_state.last_spot);

            // Apply trading fee
            let fee = self.config.fee * output_amount;

            (output_amount - fee, fee, new_state)
        }

        // Calculate target amount from curve
        fn calc_target(&self, p0: Decimal, k: Decimal, actual: Decimal, surplus: Decimal) -> Decimal {
            let radicand = dec!(1) + dec!(4) * k * surplus / p0 / actual;
            let num = (dec!(2) * k - 1 + radicand.sqrt().unwrap()) * actual;
            num / k / dec!(2)
        }

        // Calculate spot price from curve
        fn calc_spot(&self, p0: Decimal, k: Decimal, target: Decimal, actual: Decimal) -> Decimal {
            let target2 = target * target;
            let actual2 = actual * actual;

            let num = actual2 + k * (target2 - actual2);
            num / actual2 * p0
        }

        // Calculate equilibrium price from shortage and spot price
        fn calc_p0_from_spot(&self, p_spot: Decimal, k: Decimal, target: Decimal, actual: Decimal) -> Decimal {
            let target2 = target * target;
            let actual2 = actual * actual;

            let den = actual2 + k * (target2 - actual2);
            actual2 / den * p_spot
        }

        // Calculate at what price incoming trades reach equilibrium following the curve
        fn calc_p0_from_surplus(&self, surplus: Decimal, k: Decimal, target: Decimal, actual: Decimal) -> Decimal {
            // Calculate the shortage of tokens
            let shortage = target - actual;

            // Calculate the price at equilibrium (p0) using the given formula
            surplus / shortage / (dec!(1) + k * shortage / actual)
        }

        // Calculate the incoming amount of output tokens given input_amount, target, actual, and p_ref
        fn calc_incoming(
            &self,
            input_amount: Decimal,
            target: Decimal,
            actual: Decimal,
            p_ref: Decimal,
        ) -> Decimal {
            // Ensure the sum of the actual and input amounts does not exceed the target
            assert!(actual + input_amount <= target, "Infeasible amount");
            
            // Calculate the existing surplus as per AMM curve
            let surplus_before = (target - actual) * p_ref * (dec!(1) + self.config.k_in * (target - actual) / actual);

            // Calculate the new surplus as per the AMM curve
            let actual_after = actual + input_amount;
            let surplus_after = (target - actual_after) * p_ref * (dec!(1) + self.config.k_in * (target - actual_after) / actual_after);

            // The difference is the output amount
            surplus_before - surplus_after
        }

        // Calculate the outgoing amount of output tokens given input_amount, surplus, target, actual, and p_ref
        fn calc_outgoing(
            &self,
            input_amount: Decimal,
            target: Decimal,
            actual: Decimal,
            p_ref: Decimal,
        ) -> Decimal {
            // Calculate current shortage
            let shortage_before = target - actual;

            // Calculate scaled surplus and input amount using p_ref and trading curve
            let scaled_surplus_before = shortage_before * (dec!(1) + self.config.k_out * shortage_before / actual) * p_ref;
            let scaled_surplus_after = (scaled_surplus_before + input_amount) / p_ref;

            // Check if the egress price curve exponent (k_out) is 1
            if self.config.k_out == dec!(1) {
                // Calculate the shortage before and after the operation
                let shortage_after = target * scaled_surplus_after / (target + scaled_surplus_after);

                // Calculate and return the difference in shortage
                shortage_after - shortage_before
            } else {
                // Handle other values for k_out
                let shortage_after = target + scaled_surplus_after
                    - Decimal::sqrt(
                        &(target * target
                        + (dec!(4) * self.config.k_out - dec!(2)) * target * scaled_surplus_after
                        + scaled_surplus_after * scaled_surplus_after)
                    ).unwrap();

                // Calculate and return the difference in shortage
                // TODO: fix shortage_before scaling
                (shortage_after - shortage_before) / dec!(2) / (dec!(1) - self.config.k_out)
            }
        }
    }
}
