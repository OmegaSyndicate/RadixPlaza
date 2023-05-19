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

        pub fn create_pair(&mut self, tokens: Bucket, dfp2: Bucket, p0: Decimal) -> (Bucket, Bucket) {
            // Verify dfp2 tokens are indeed dfp2 tokens
            assert!(dfp2.resource_address() == self.dfp2, "Need to add DFP2 liquidity");
            
            // Ensure the pair doesn't exist yet
            let rri = tokens.resource_address();
            assert!(self.token_to_pair.contains_key(&rri) == false, "Pair already exists");
        
            // Create new pair
            let (pair, lp_base, lp_quote) = PlazaPairComponent::instantiate_pair(tokens, dfp2, p0);

            // Add new pair to database
            self.token_to_pair.insert(rri, pair);
            self.lp_to_token.insert(lp_base.resource_address(), rri);
            self.lp_to_token.insert(lp_quote.resource_address(), rri);

            (lp_base, lp_quote)
        }

        pub fn quote(&self, input_token: ResourceAddress, input_amount: Decimal, output_token: ResourceAddress) -> Decimal {
            // Verify tokens are all traded at the exchange
            assert!(input_token != output_token, "Can't swap token into itself");
            assert!(self.token_to_pair.contains_key(&input_token) || input_token == self.dfp2, "Input token not listed");
            assert!(self.token_to_pair.contains_key(&output_token) || output_token == self.dfp2, "Output token not listed");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, _) => {
                    // Sell DFP2 from a single pair
                    let pair = &self.token_to_pair[&output_token];
                    pair.quote(input_amount, true).0
                }
                (_, true) => {
                    // Buy DFP2 to a single pair
                    let pair = &self.token_to_pair[&input_token];
                    pair.quote(input_amount, false).0
                }
                _ => {
                    // Trade using a hop through DFP2
                    let pair1 = &self.token_to_pair[&input_token];
                    let pair2 = &self.token_to_pair[&output_token];
                    let dfp2_amount = pair1.quote(input_amount, false).0;
                    pair2.quote(dfp2_amount, true).0
                }
            }            
        }
   }
}