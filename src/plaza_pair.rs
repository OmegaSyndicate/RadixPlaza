use scrypto::prelude::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum PairState {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct PairParams {
    p0: Decimal,                // Equilibrium price
    base_target: Decimal,       // Target amount of base tokens
    quote_target: Decimal,      // Target amount of quote tokens
    state: PairState,           // Current state of the pair
    k_in: Decimal,              // Ingress price curve exponent
    k_out: Decimal,             // Egress price curve exponent
    fee: Decimal,               // Trading fee
    last_trade: i64,            // Timestamp of last trade
    last_outgoing: i64,         // Timestamp of last outgoing trade
    last_out_spot: Decimal,     // Last outgoing spot price
}

impl fmt::Display for PairState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PairState::BaseShortage => write!(f, "BaseShortage"),
            PairState::Equilibrium => write!(f, "Equilibrium"),
            PairState::QuoteShortage => write!(f, "QuoteShortage"),
        }
    }
}

#[blueprint]
mod plazapair {
    // PlazaPair struct represents a liquidity pair in the trading platform
    struct PlazaPair { 
        params: PairParams,                 // Pool parameters
        base_vault: Vault,                  // Holds the base tokens
        quote_vault: Vault,                 // Holds the quote tokens
        base_lp: ResourceManager,           // Resource address of base LP tokens
        quote_lp: ResourceManager,          // Resource address of quote LP tokens
    }

    impl PlazaPair {
        // Instantiate a new Plaza style trading pair
        pub fn instantiate_pair(
            base_token: ResourceManager,
            quote_token: ResourceManager,
            initial_price: Decimal,
        ) -> Global<PlazaPair> {
            // Ensure both tokens are fungible
            assert!(base_token.resource_type().is_fungible(), "non-fungible base token detected");
            assert!(quote_token.resource_type().is_fungible(), "non-fungible quote token detected");

            // Reserve address for Actor Virtual Badge
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Runtime::blueprint_id());

            // Create LP tokens for the base token providers, starting at 1:1
            let base_lp: ResourceManager = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata! {
                    init {
                        "name" => "PlazaPair Base LP", locked;
                        "symbol" => "PLAZALP", locked;
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

            // Create LP tokens for the quote token providers, starting at 1:1
            let quote_lp: ResourceManager = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata! {
                    init {
                        "name" => "PlazaPair Quote LP", locked;
                        "symbol" => "PLAZALP", locked;
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
                params: PairParams {
                    p0: initial_price,
                    base_target: dec!(0),
                    quote_target: dec!(0),
                    state: PairState::Equilibrium,
                    k_in: dec!("0.4"),
                    k_out: dec!("1"),
                    fee: dec!("0.003"),
                    last_trade: now,
                    last_outgoing: now,
                    last_out_spot: initial_price,
                },
                base_vault: Vault::new(base_token.address()),
                quote_vault: Vault::new(quote_token.address()),
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
            // Ensure the bucket is not empty
            assert!(input_bucket.amount() > dec!(0), "Empty bucket provided");
        
            // Determine if the bucket is for the quote or the base pool
            let is_quote = self.quote_vault.resource_address() == input_bucket.resource_address();
        
            // Based on the bucket type, choose the correct vault, target and resource address
            let (vault, mut target_value, lp_manager) = if is_quote {
                (&mut self.quote_vault, self.params.quote_target, self.quote_lp)
            } else {
                (&mut self.base_vault, self.params.base_target, self.base_lp)
            };
        
            // Check amount of outstanding LP tokens
            let lp_outstanding = lp_manager.total_supply().unwrap();
        
            // Calculate the new LP amount
            let new_lp_value = if lp_outstanding == dec!(0) { 
                // If the LP supply is zero, the new LP value is the input amount
                input_bucket.amount()
            } else {
                // Otherwise, it's calculated based on the input amount, target and LP supply
                input_bucket.amount() / target_value * lp_outstanding
            };
        
            // Add the new liquidity to the target and deposit into the vault
            target_value += input_bucket.amount();
            vault.put(input_bucket);

            // Mint the new LP tokens and return to the caller
            lp_manager.mint(new_lp_value)
        }

        // Exchange LP tokens for the underlying liquidity held in the pair
        // TODO -- RESET TARGETS ON PRICE X-OVER
        // TODO -- ENSURE HEALTH WITH ZERO LIQ
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            let is_quote = lp_tokens.resource_address() == self.quote_lp.address();

            // Determine which vault and target values should be used
            let (this_vault, other_vault, this_target, other_target, is_shortage, lp_manager) = if is_quote {
                (
                    &mut self.quote_vault,
                    &mut self.base_vault,
                    &mut self.params.quote_target,
                    self.params.base_target,
                    self.params.state == PairState::QuoteShortage,
                    self.quote_lp,
                )
            } else {
                (
                    &mut self.base_vault,
                    &mut self.quote_vault,
                    &mut self.params.base_target,
                    self.params.quote_target,
                    self.params.state == PairState::BaseShortage,
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
            // Determine if the input tokens are quote tokens or base tokens.
            let is_quote = input_tokens.resource_address() == self.quote_vault.resource_address();

            // Calculate the amount of output tokens and pair impact variables.
            let (output_amount, fee, mut new_params) = self.quote(input_tokens.amount(), is_quote);

            // Log trade event
            let (token_in, token_out) = if is_quote {
                ("quote", "base")
            } else {
                ("base", "quote")
            };
            info!("SWAP: {} {} for {} {}.", input_tokens.amount(), token_in, output_amount, token_out);

            // TODO: fee sharing
            // Update the target values and select the input and output vaults based on input_tokens type.
            let (input_vault, output_vault) = if is_quote {
                new_params.base_target += fee;
                (&mut self.quote_vault, &mut self.base_vault)
            } else {
                new_params.quote_target += fee;
                (&mut self.base_vault, &mut self.quote_vault)
            };

            // Adjust pair state variables.
            self.params = new_params;
            debug!("p0: {} -- state {}", new_params.p0, new_params.state);

            // Transfer the tokens.
            input_vault.put(input_tokens);
            output_vault.take(output_amount)
        }

        // Getter function to identify related LP tokens
        pub fn get_lp_tokens(&self) -> (ResourceManager, ResourceManager) {
            (self.base_lp, self.quote_lp)
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(&self, input_amount: Decimal, input_is_quote: bool) -> (Decimal, Decimal, PairParams) {
            let mut new_params = self.params;

            // Compute time since previous trade
            let t = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            let delta_t = if t > self.params.last_trade { 
                t - self.params.last_trade
            } else { 
                0
            };
            new_params.last_trade = t;

            // Compute decay factor with a 15 minute time constant
            let factor = Decimal::powi(&dec!("0.9355"), delta_t / 60);

            // Collect current actual balances
            let (base_actual, quote_actual) = (self.base_vault.amount(), self.quote_vault.amount());

            // Set the input and output vaults and targets based on input_tokens type
            let (input_actual, mut output_actual, input_target, output_target) =
                if input_is_quote {
                    (
                        quote_actual,
                        base_actual,
                        self.params.quote_target,
                        self.params.base_target,
                    )
                } else {
                    (
                        base_actual,
                        quote_actual,
                        self.params.base_target,
                        self.params.quote_target,
                    )
                };            

            // Calculate the base[quote] price in equilibrium
            let new_p0 = match self.params.state {
                PairState::BaseShortage => {
                    let quote_surplus = quote_actual - self.params.quote_target;
                    self.calc_p0(quote_surplus, self.params.base_target, base_actual)
                }
                PairState::Equilibrium => self.params.p0,
                PairState::QuoteShortage => {
                    let base_surplus = base_actual - self.params.base_target;
                    dec!(1) / self.calc_p0(base_surplus, self.params.quote_target, quote_actual)
                }
            };
            let p0 = factor * new_p0 + (dec!(1) - factor) * self.params.p0;
            new_params.p0 = p0;

            // Set reference price to B[Q] or Q[B] depending on liquidity
            let mut p_ref = match self.params.state {
                PairState::BaseShortage => p0,
                PairState::Equilibrium => if input_is_quote { p0 } else { dec!(1) / p0 },
                PairState::QuoteShortage => dec!(1) / p0
            };

            // Determine which state represents input shortage based on input type
            let input_shortage = match input_is_quote {
                true => PairState::QuoteShortage,
                false => PairState::BaseShortage
            };

            // Define running counters
            let mut amount_to_trade = input_amount;
            let mut output_amount = dec!(0);

            // Handle the incoming case (trading towards equilibrium)
            if new_params.state == input_shortage {
                if amount_to_trade + input_actual >= input_target {
                    // Trading past equilibrium
                    amount_to_trade -= input_target - input_actual;
                    output_amount = output_actual - output_target;

                    // Update variables for second part of trade
                    new_params.state = PairState::Equilibrium;
                    output_actual = output_target;
                    p_ref = dec!(1) / p_ref;
                } else {
                    // Trading towards but short of equilibrium
                    output_amount = self.calc_incoming(
                        amount_to_trade,
                        input_target,
                        input_actual,
                        p_ref,
                    );
                }
            }

            // Handle the trading away from equilbrium case
            if new_params.state != input_shortage && amount_to_trade > dec!(0) {
                // Calibrate outgoing price curve to incoming at spot price.
                let target2 = output_target * output_target;
                let actual2 = output_actual * output_actual;
                let num_new = (dec!(1) - factor) * (actual2 + self.params.k_in * (target2 - actual2));
                let num_old = factor * actual2 * self.params.last_out_spot;
                let den = actual2 + self.params.k_out * (target2 - actual2);
                let virtual_p_ref = (num_new + num_old) / den * p_ref;
                
                // Calculate output amount based on outgoing curve
                let outgoing_output = self.calc_outgoing(
                    amount_to_trade,
                    output_target,
                    output_actual,
                    virtual_p_ref,
                );
                output_amount += outgoing_output;

                // Select what the new exchange state should be
                new_params.state = match input_shortage {
                    PairState::QuoteShortage => PairState::BaseShortage,
                    PairState::BaseShortage => PairState::QuoteShortage,
                    PairState::Equilibrium => {
                        if input_is_quote {
                            PairState::BaseShortage
                        } else {
                            PairState::QuoteShortage
                        }
                    }
                };

                // Store previous outgoing time stamp
                new_params.last_outgoing = t;

                // Calculate and store previous outgoing spot price
                let new_actual = output_actual - outgoing_output;
                let new_actual2 = new_actual * new_actual;
                new_params.last_out_spot = match new_params.state {
                    PairState::BaseShortage => (dec!(1) + new_params.k_out * (target2 - new_actual2) / new_actual2) * virtual_p_ref,
                    PairState::Equilibrium => new_params.p0,
                    PairState::QuoteShortage => dec!(1) / (dec!(1) + new_params.k_out * (target2 - new_actual2) / new_actual2) * virtual_p_ref, 
                };
            }

            // Apply trading fee
            let fee = self.params.fee * output_amount;

            (output_amount - fee, fee, new_params)
        }

        // Calculate the price at equilibrium (p0) given surplus, target, and actual token amounts
        fn calc_p0(&self, surplus: Decimal, target: Decimal, actual: Decimal) -> Decimal {
            // Calculate the shortage of tokens
            let shortage = target - actual;

            // Calculate the price at equilibrium (p0) using the given formula
            surplus / shortage / (dec!(1) + self.params.k_in * shortage / actual)
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
            let surplus_before = (target - actual) * p_ref * target / actual;

            // Calculate the new surplus as per the AMM curve
            let actual_after = actual + input_amount;
            let surplus_after = (target - actual_after) * p_ref * target / actual_after;

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
            let scaled_surplus_before = shortage_before * (dec!(1) + self.params.k_out * shortage_before / actual) * p_ref;
            let scaled_surplus_after = (scaled_surplus_before + input_amount) / p_ref;

            // Check if the egress price curve exponent (k_out) is 1
            if self.params.k_out == dec!(1) {
                // Calculate the shortage before and after the operation
                let shortage_after = target * scaled_surplus_after / (target + scaled_surplus_after);

                // Calculate and return the difference in shortage
                shortage_after - shortage_before
            } else {
                // Handle other values for k_out
                let shortage_after = target + scaled_surplus_after
                    - Decimal::sqrt(
                        &(target * target
                        + (dec!(4) * self.params.k_out - dec!(2)) * target * scaled_surplus_after
                        + scaled_surplus_after * scaled_surplus_after)
                    ).unwrap();

                // Calculate and return the difference in shortage
                // TODO: fix shortage_before scaling
                (shortage_after - shortage_before) / dec!(2) / (dec!(1) - self.params.k_out)
            }
        }
    }
}
