use scrypto::prelude::*;
use crate::types::PairConfig;
use crate::pair::plazapair::PlazaPair;

#[blueprint]
mod plazadex {
    // PlazaDex is the DefiPlaza decentralized exchange on Radix
    struct PlazaDex {
        // Pair location for certain token / lp_token
        dfp2: ResourceAddress,
        token_to_pair: KeyValueStore<ResourceAddress, Global<PlazaPair>>,
        lp_to_token: KeyValueStore<ResourceAddress, ResourceAddress>,
        dfp2_reserves: KeyValueStore<ResourceAddress, Vault>,
        min_dfp2_liquidity: Decimal,
    }

    impl PlazaDex {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_dex(dfp2_address: ResourceAddress) -> Global<PlazaDex> {
            // Instantiate a PlazaDex component
            Self {
                dfp2: dfp2_address,
                token_to_pair: KeyValueStore::new(),
                lp_to_token: KeyValueStore::new(),
                dfp2_reserves: KeyValueStore::new(),
                min_dfp2_liquidity: dec!(0),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::None)
            .globalize()
        }

        // Create a new liquidity pair on the exchange
        pub fn create_pair(&mut self, token: ResourceAddress, dfp2: Bucket, p0: Decimal) -> Global<PlazaPair> {
            // Ensure all basic criteria are met to add a new pair
            assert!(dfp2.resource_address() == self.dfp2, "Need to add DFP2 liquidity");
            assert!(dfp2.amount() >= self.min_dfp2_liquidity, "Insufficient DFP2 liquidity");
            assert!(self.token_to_pair.get(&token).is_some(), "Pair already exists");
            assert!(token != self.dfp2, "Can't add DFP2 as base token");
            
            // Instantiate new pair
            let config = PairConfig {
                k_in: dec!("0.4"),
                k_out: dec!("1"),
                fee: dec!("0.003"),
            };
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

        // Remove liquidity
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            let lp_address = lp_tokens.resource_address();
            let base_address = self.lp_to_token.get(&lp_address).expect("Unknown LP token");
            let pair = self.token_to_pair.get_mut(&base_address).expect("Pair not found");
            pair.remove_liquidity(lp_tokens)
        }

        // Get a quote for swapping tokens
        pub fn quote(&self, input_token: ResourceAddress, input_amount: Decimal, output_token: ResourceAddress) -> Decimal {
            // Verify tokens are all traded at the exchange
            assert!(input_token != output_token, "Can't swap token into itself");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, _) => {
                    // Sell DFP2 (single pair trade)
                    let pair = self.token_to_pair.get(&output_token).expect("Output token not listed");
                    pair.quote(input_amount, true).0
                }
                (_, true) => {
                    // Buy DFP2 (single pair trade)
                    let pair = self.token_to_pair.get(&input_token).expect("Input token not listed");
                    pair.quote(input_amount, false).0
                }
                _ => {
                    // Trade two tokens with a hop through DFP2
                    let pair1 = self.token_to_pair.get(&input_token).expect("Input token not listed");
                    let dfp2_amount = pair1.quote(input_amount, false).0;
                    let pair2 = self.token_to_pair.get(&output_token).expect("Output token not listed");
                    pair2.quote(dfp2_amount, true).0
                }
            }            
        }

        pub fn get_lp_tokens(&self, base_token: ResourceAddress) -> (ResourceAddress, ResourceAddress) {
            let pair = self.token_to_pair.get(&base_token).expect("Token not listed");
            pair.get_lp_tokens()
        } 
   }
}