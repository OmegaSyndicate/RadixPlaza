use scrypto::prelude::*;
use crate::events::*;
use crate::types::PairConfig;
use crate::pair::plazapair::PlazaPair;

#[blueprint]
#[events(PairCreated, TokenDeListed, TokenBlacklisted, TokenDeBlacklisted)]
mod plazadex {
    enable_method_auth! { 
        methods { 
            create_pair => PUBLIC;
            swap => PUBLIC;
            add_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            quote => PUBLIC;
            get_lp_tokens => PUBLIC;
            delist => restrict_to: [OWNER];
            blacklist => restrict_to: [OWNER];
            deblacklist => restrict_to: [OWNER];
        }
    }

    // PlazaDex is the DefiPlaza decentralized exchange on Radix
    struct PlazaDex {
        // Pair location for certain token / lp_token
        dfp2: ResourceAddress,
        blacklist: HashSet<ResourceAddress>,
        token_to_pair: KeyValueStore<ResourceAddress, Global<PlazaPair>>,
        lp_to_token: KeyValueStore<ResourceAddress, ResourceAddress>,
        dfp2_reserves: KeyValueStore<ResourceAddress, Vault>,
        min_dfp2_liquidity: Decimal,
    }

    impl PlazaDex {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_dex(dfp2_address: ResourceAddress, owner_badge: ResourceAddress) -> Global<PlazaDex> {
            // Instantiate a PlazaDex component
            Self {
                dfp2: dfp2_address,
                blacklist: HashSet::new(),
                token_to_pair: KeyValueStore::new(),
                lp_to_token: KeyValueStore::new(),
                dfp2_reserves: KeyValueStore::new(),
                min_dfp2_liquidity: dec!(0),
            }
            .instantiate()
            .prepare_to_globalize(
                OwnerRole::Fixed(
                    rule!(require(owner_badge))
                )
            )
            .globalize()
        }

        // Create a new liquidity pair on the exchange
        pub fn create_pair(&mut self, token: ResourceAddress, dfp2: Bucket, config: PairConfig, p0: Decimal) -> Global<PlazaPair> {
            // Ensure all basic criteria are met to add a new pair
            assert!(!self.blacklist.contains(&token), "Token is blacklisted");
            assert!(dfp2.resource_address() == self.dfp2, "Need to add DFP2 liquidity");
            assert!(dfp2.amount() >= self.min_dfp2_liquidity, "Insufficient DFP2 liquidity");
            assert!(self.token_to_pair.get(&token).is_none(), "Pair already exists");
            assert!(token != self.dfp2, "Can't add DFP2 as base token");
            
            // Instantiate new pair
            let pair = PlazaPair::instantiate_pair(token, self.dfp2, config, p0);
            let (lp_base, lp_quote) = pair.get_lp_tokens();

            // Add new pair to database
            self.token_to_pair.insert(token, pair);
            self.lp_to_token.insert(lp_base, token);
            self.lp_to_token.insert(lp_quote, token);

            if dfp2.amount() > dec!(0) {
                let lp_tokens = pair.add_liquidity(dfp2);
                let dfp2_vault = Vault::with_bucket(lp_tokens);
                self.dfp2_reserves.insert(token, dfp2_vault);
            }

            Runtime::emit_event(PairCreated{base_token: token, config, p0, component: pair});

            pair
        }

        // Swap tokens
        pub fn swap(&mut self, tokens: Bucket, output_token: ResourceAddress) -> Bucket {
            let input_token = tokens.resource_address();

            // Verify tokens can be traded at the exchange
            assert!(input_token != output_token, "Can't swap token into itself");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, _) => {
                    // Sell DFP2 (single pair trade)
                    let pair = self.token_to_pair.get_mut(&output_token).expect("Output token not listed");
                    pair.swap(tokens)
                }
                (_, true) => {
                    // Buy DFP2 (single pair trade)
                    let pair = self.token_to_pair.get_mut(&input_token).expect("Input token not listed");
                    pair.swap(tokens)
                }
                _ => {
                    // Trade two tokens with a hop through DFP2
                    let dfp2_bucket;
                    {
                        let pair1 = self.token_to_pair.get_mut(&input_token).expect("Input token not listed");
                        dfp2_bucket = pair1.swap(tokens);
                    }
                    let pair2 = self.token_to_pair.get_mut(&output_token).expect("Output token not listed");
                    pair2.swap(dfp2_bucket)
                }
            }
        }

        // Add liquidity to the exchange
        pub fn add_liquidity(&mut self, tokens: Bucket, base_token: Option<ResourceAddress>) -> Bucket {
            let input_token = tokens.resource_address();

            if input_token == self.dfp2 {
                // Verify base token is provided and listed
                let base_token = base_token.expect("No base token provided");

                // Find corresponding pair and add liquidity
                let pair = self.token_to_pair.get_mut(&base_token).expect("Base token not listed");
                pair.add_liquidity(tokens)
            } else {
                // Find corresponding pair and add liquidity
                let pair = self.token_to_pair.get_mut(&input_token).expect("Input token not listed");
                pair.add_liquidity(tokens)
            }
        }

        // Remove liquidity from the exchange
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            let lp_address = lp_tokens.resource_address();
            let base_address = self.lp_to_token.get(&lp_address).expect("Unknown LP token");
            let pair = self.token_to_pair.get_mut(&base_address).expect("Pair not found");
            pair.remove_liquidity(lp_tokens)
        }

        // Get a quote for swapping two tokens
        pub fn quote(&self, input_token: ResourceAddress, input_amount: Decimal, output_token: ResourceAddress) -> Decimal {
            // Verify tokens are all traded at the exchange
            assert!(input_token != output_token, "Can't swap token into itself");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, true) => { Runtime::panic("DFP2 <--> DFP2".to_string()) }
                (true, false) => {
                    // Sell DFP2 (single pair trade)
                    let pair = self.token_to_pair.get(&output_token).expect("Output token not listed");
                    pair.quote(input_amount, true).0
                }
                (false, true) => {
                    // Buy DFP2 (single pair trade)
                    let pair = self.token_to_pair.get(&input_token).expect("Input token not listed");
                    pair.quote(input_amount, false).0
                }
                (false, false) => {
                    // Trade two tokens with a hop through DFP2
                    let pair1 = self.token_to_pair.get(&input_token).expect("Input token not listed");
                    let dfp2_amount = pair1.quote(input_amount, false).0;
                    let pair2 = self.token_to_pair.get(&output_token).expect("Output token not listed");
                    pair2.quote(dfp2_amount, true).0
                }
            }            
        }

        // Read only method returning the LP tokens associated with the pair for a given base token.
        pub fn get_lp_tokens(&self, base_token: ResourceAddress) -> (ResourceAddress, ResourceAddress) {
            let pair = self.token_to_pair.get(&base_token).expect("Token not listed");
            pair.get_lp_tokens()
        }

        // Removes a currently listed token pair. This prevents it from being routed to by the DEX
        // but it remains available on the ledger to be called directly.
        pub fn delist(&mut self, base_token: ResourceAddress) {
            let pair = self.token_to_pair.get(&base_token).expect("Token not listed");
            let (base_lp, quote_lp) = pair.get_lp_tokens();
            
            self.lp_to_token.remove(&base_lp);
            self.lp_to_token.remove(&quote_lp);
            self.token_to_pair.remove(&base_token);

            Runtime::emit_event(TokenDeListed{base_token, component: *pair});
        }

        // Blacklists a token so it can't be added to the DEX again. Delists the token should it
        // currently be listed.
        pub fn blacklist(&mut self, token: ResourceAddress) {
            assert!(!self.blacklist.contains(&token), "Token already blacklisted");
            if self.token_to_pair.get(&token).is_some() {
                self.delist(token);
            }
            self.blacklist.insert(token);

            Runtime::emit_event(TokenBlacklisted{token});
        }

        // Removes a token from the blacklist so it can once again be added to the exchange
        pub fn deblacklist(&mut self, token: ResourceAddress) {
            assert!(self.blacklist.contains(&token), "Token not blacklisted");
            self.blacklist.remove(&token);

            Runtime::emit_event(TokenDeBlacklisted{token});
        }
   }
}