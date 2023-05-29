use scrypto::prelude::*;
use crate::plaza_pair::plazapair::*;

#[blueprint]
mod plazadex {
    // PlazaDex is the DefiPlaza decentralized exchange on Radix
    struct PlazaDex {
        // Pair location for certain token / lp_token
        dfp2: ResourceAddress,
        token_to_pair: HashMap<ResourceAddress, PlazaPairComponent>,
        lp_to_token: HashMap<ResourceAddress, ResourceAddress>,
    }

    impl PlazaDex {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_dex(dfp2_address: ResourceAddress) -> ComponentAddress {
            // Instantiate a PlazaDex component
            Self {
                dfp2: dfp2_address,
                token_to_pair: HashMap::new(),
                lp_to_token: HashMap::new(),
            }
            .instantiate()
            .globalize()
        }

        // Create a new liquidity pair on the exchange
        pub fn create_pair(&mut self, tokens: Bucket, dfp2: Bucket, p0: Decimal) -> (Bucket, Bucket) {
            // Verify dfp2 tokens are indeed dfp2 tokens
            assert!(dfp2.resource_address() == self.dfp2, "Need to add DFP2 liquidity");
            
            // Ensure the pair doesn't exist yet
            let rri = tokens.resource_address();
            assert!(self.token_to_pair.contains_key(&rri) == false, "Pair already exists");
            assert!(rri != self.dfp2, "Can't add DFP2 as base token");
            
            // Create new pair
            let (pair, lp_base, lp_quote) = PlazaPairComponent::instantiate_pair(tokens, dfp2, p0);

            // Add new pair to database
            self.token_to_pair.insert(rri, pair);
            self.lp_to_token.insert(lp_base.resource_address(), rri);
            self.lp_to_token.insert(lp_quote.resource_address(), rri);

            (lp_base, lp_quote)
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
                    let pair1 = self.token_to_pair.get_mut(&input_token).expect("Input token not listed");
                    let dfp2_bucket = pair1.swap(tokens);
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