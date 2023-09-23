use scrypto::prelude::*;
use crate::constants::*;
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
            update_lp_metadata => restrict_to: [OWNER];
            withdraw_owned_liquidity => restrict_to: [OWNER];
            set_min_dfp2 => restrict_to: [OWNER];
        }
    }

    // PlazaDex is the DefiPlaza decentralized exchange on Radix
    struct PlazaDex {
        // Pair location for certain token / lp_token
        dfp2: ResourceAddress,
        blacklist: HashSet<ResourceAddress>,
        address_to_pair: KeyValueStore<ResourceAddress, Global<PlazaPair>>,
        pair_to_lps: KeyValueStore<ComponentAddress, (ResourceAddress, ResourceAddress)>,
        dex_reserves: KeyValueStore<ComponentAddress, (Vault, Vault)>,
        min_dfp2_liquidity: Decimal,
        pairs_owner: OwnerRole,
    }

    impl PlazaDex {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_dex(dfp2_address: ResourceAddress, owner_badge: ResourceAddress) -> Global<PlazaDex> {
            // Reserve address for Actor Virtual Badge
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(PlazaDex::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);

            // Instantiate a PlazaDex component
            Self {
                dfp2: dfp2_address,
                blacklist: HashSet::new(),
                address_to_pair: KeyValueStore::new(),
                pair_to_lps: KeyValueStore::new(),
                dex_reserves: KeyValueStore::new(),
                min_dfp2_liquidity: dec!(0),
                pairs_owner: OwnerRole::Fixed(rule!(require(global_component_caller_badge))),
            }
            .instantiate()
            .prepare_to_globalize(
                OwnerRole::Fixed(
                    rule!(require(owner_badge))
                )
            )
            .with_address(address_reservation)
            .globalize()
        }

        // Create a new liquidity pair on the exchange
        pub fn create_pair(
            &mut self,
            mut base_bucket: Bucket,
            mut dfp2_bucket: Bucket,
            config: PairConfig,
            p0: Decimal,
        ) -> Global<PlazaPair> {
            let token = base_bucket.resource_address();

            // Ensure all basic criteria are met to add a new pair
            assert!(!self.blacklist.contains(&token), "Token is blacklisted");
            assert!(base_bucket.amount() >= self.min_dfp2_liquidity / p0, "Insufficient base liquidity");
            assert!(dfp2_bucket.resource_address() == self.dfp2, "Need to add DFP2 liquidity");
            assert!(dfp2_bucket.amount() >= self.min_dfp2_liquidity, "Insufficient DFP2 liquidity");
            assert!(self.address_to_pair.get(&token).is_none(), "Pair already exists");
            assert!(token != self.dfp2, "Can't add DFP2 as base token");
            
            // Instantiate new pair
            let tiny_base_bucket = base_bucket.take(MIN_LIQUIDITY);
            let tiny_dfp2_bucket = dfp2_bucket.take(MIN_LIQUIDITY);
            let pair = PlazaPair::instantiate_pair(
                self.pairs_owner.clone(),
                tiny_base_bucket,
                tiny_dfp2_bucket,
                config,
                p0
            );

            // Add liquidity to new pair
            let base_lp_bucket = pair.add_liquidity(base_bucket.into(), false);
            let dfp2_lp_bucket = pair.add_liquidity(dfp2_bucket.into(), true);
            let base_lp_address = base_lp_bucket.resource_address();
            let dfp2_lp_address = dfp2_lp_bucket.resource_address();

            // Set name for LP tokens
            let base_manager = ResourceManager::from(token);
            let symbol = base_manager.get_metadata("symbol")
                .unwrap_or(Some("XXXXX".to_owned())).unwrap_or("XXXXX".to_owned());
            let base_name = format!("Defiplaza {} Base", symbol);
            let quote_name = format!("Defiplaza {} Quote", symbol);
            let base_icon = format!("https://assets.defiplaza.net/lptokens/{}_base.png", base_manager.address().to_hex());
            let quote_icon = format!("https://assets.defiplaza.net/lptokens/{}_quote.png", base_manager.address().to_hex());

            // Assign metadata
            let base_lp_manager = ResourceManager::from(base_lp_address);
            let dfp2_lp_manager = ResourceManager::from(dfp2_lp_address);
            base_lp_manager.set_metadata("symbol", "BASELP".to_owned());
            dfp2_lp_manager.set_metadata("symbol", "DFP2LP".to_owned());
            base_lp_manager.set_metadata("name", base_name);
            dfp2_lp_manager.set_metadata("name", quote_name);
            base_lp_manager.set_metadata("icon_url", base_icon);
            dfp2_lp_manager.set_metadata("icon_url", quote_icon);

            // Store DEX reserves
            let pair_address = pair.address();
            let base_lp_vault = Vault::with_bucket(base_lp_bucket);
            let dfp2_lp_vault = Vault::with_bucket(dfp2_lp_bucket);
            self.dex_reserves.insert(pair_address, (base_lp_vault, dfp2_lp_vault));

            // Add new pair to database
            self.address_to_pair.insert(token, pair);
            self.address_to_pair.insert(base_lp_address, pair);
            self.address_to_pair.insert(dfp2_lp_address, pair);
            self.pair_to_lps.insert(pair_address, (base_lp_address, dfp2_lp_address));

            // Emit pair creation event
            Runtime::emit_event(PairCreated{base_token: token, config, p0, component: pair});

            pair
        }

        // Swap tokens
        pub fn swap(&mut self, tokens: Bucket, output_token: ResourceAddress) -> (Bucket, Option<Bucket>) {
            let input_token = tokens.resource_address();
            assert!(input_token != output_token, "Can't swap token into itself");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, _) => {
                    // Sell DFP2 (single pair trade)
                    let pair = self.address_to_pair.get_mut(&output_token).expect("Output token not listed");
                    pair.swap(tokens)
                }
                (_, true) => {
                    // Buy DFP2 (single pair trade)
                    let pair = self.address_to_pair.get_mut(&input_token).expect("Input token not listed");
                    pair.swap(tokens)
                }
                _ => {
                    // Trade two tokens with a hop through DFP2
                    let (dfp2_bucket, remainder);
                    {
                        let pair1 = self.address_to_pair.get_mut(&input_token).expect("Input token not listed");
                        (dfp2_bucket, remainder) = pair1.swap(tokens);
                    }

                    // Second hop, separate scope due to mutability rules
                    let (output_bucket, dfp2_returned);
                    {
                        let pair2 = self.address_to_pair.get_mut(&output_token).expect("Output token not listed");
                        (output_bucket, dfp2_returned) = pair2.swap(dfp2_bucket);
                    }
                    
                    // Swap back the dfp2 that we couldn't trade
                    let change_bucket = dfp2_returned.and_then(|bucket| {
                        let pair1 = self.address_to_pair.get_mut(&input_token).unwrap();
                        let (change, _) = pair1.swap(bucket);
                        Some(change)
                    });

                    // Combine returned buckets
                    match (remainder, change_bucket) {
                        (Some(bucket1), Some(mut bucket2)) => {
                            bucket2.put(bucket1);
                            (output_bucket, Some(bucket2))
                        },
                        (Some(bucket1), _) => (output_bucket, Some(bucket1)),
                        (_, Some(bucket2)) => (output_bucket, Some(bucket2)),
                        _ => (output_bucket, None),
                    }
                }
            }
        }

        // Add liquidity to the exchange
        pub fn add_liquidity(&mut self, tokens: Bucket, base_token: Option<ResourceAddress>) -> Bucket {
            let input_token = tokens.resource_address();
            let is_quote = input_token == self.dfp2;

            // Select liquidity pair from database
            let pair = match (!is_quote, base_token) {
                (true, _) => self.address_to_pair.get_mut(&input_token).expect("Input token not listed"),
                (false, Some(token)) => self.address_to_pair.get_mut(&token).expect("Base token not listed"),
                (false, None) => Runtime::panic("No base token provided".to_string()),
            };

            // Add liquidity and return output
            pair.add_liquidity(tokens, is_quote)
        }

        // Remove liquidity from the exchange
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            // Select liquidity pair from database
            let lp_address = lp_tokens.resource_address();
            let pair = self.address_to_pair.get_mut(&lp_address).expect("Unknown LP token");
            let is_quote = lp_tokens.resource_address() == self.pair_to_lps.get(&pair.address()).expect("Pair not found").1;

            // Remove liquidity from pair and return to caller
            pair.remove_liquidity(lp_tokens, is_quote)
        }

        // Get a quote for swapping two tokens
        pub fn quote(&self, input_token: ResourceAddress, input_amount: Decimal, output_token: ResourceAddress) -> Decimal {
            // Verify tokens are all traded at the exchange
            assert!(input_token != output_token, "Can't swap token into itself");

            match (input_token == self.dfp2, output_token == self.dfp2) {
                (true, true) => Runtime::panic("DFP2 <--> DFP2 makes no sene".to_string()),
                (true, false) => {
                    // Sell DFP2 (single pair trade)
                    let pair = self.address_to_pair.get(&output_token).expect("Output token not listed");
                    pair.quote(input_amount, true).0
                },
                (false, true) => {
                    // Buy DFP2 (single pair trade)
                    let pair = self.address_to_pair.get(&input_token).expect("Input token not listed");
                    pair.quote(input_amount, false).0
                },
                (false, false) => {
                    // Trade two tokens with a hop through DFP2
                    let pair1 = self.address_to_pair.get(&input_token).expect("Input token not listed");
                    let dfp2_amount = pair1.quote(input_amount, false).0;
                    let pair2 = self.address_to_pair.get(&output_token).expect("Output token not listed");
                    pair2.quote(dfp2_amount, true).0
                },
            }
        }

        /// The `get_lp_tokens` function is a read-only method designed to retrieve the Liquidity Pool (LP) tokens 
        /// associated with a token pair for a provided base token. It fetches the token pair for the provided base
        /// token, and then returns the corresponding LP tokens.
        ///
        /// # Arguments
        ///
        /// * `base_token`: A `ResourceAddress` that represents the base token of the pair.
        ///
        /// # Returns
        ///
        /// * Returns a tuple `(ResourceAddress, ResourceAddress)`, signifying the LP tokens associated with the
        ///   token pair.
        ///
        /// # Panics
        ///
        /// * The function will panic if the provided base token is not listed, or if the associated token pair
        ///   is not found in the lp token key-value store.
        pub fn get_lp_tokens(&self, base_token: ResourceAddress) -> (ResourceAddress, ResourceAddress) {
            let pair = self.address_to_pair.get(&base_token).expect("Token not listed");
            *self.pair_to_lps.get(&pair.address()).expect("Pair not found")
        }

        /// The `delist` function removes a listed token pair from the database, thereby preventing the DEX from
        /// routing through it. Despite this, the token remains available on the ledger for conducting direct
        /// transactions. This function first retrieves the token pair and its associated liquidity pool (LP)
        /// tokens using the provided base token. Then, it removes the pair and LP tokens from the internal
        /// address-to-pair mapping.
        ///
        /// # Arguments
        ///
        /// * `base_token`: The `ResourceAddress` representing the base token of the pair to be delisted.
        ///
        /// # Panics
        ///
        /// * The function will panic if the provided base token is not found in the list of token pairs.
        ///
        /// # Events
        ///
        /// * The function emits a `TokenDeListed` event after successfully delisting the token pair.
        pub fn delist(&mut self, base_token: ResourceAddress) {
            let pair = self.address_to_pair.get(&base_token).expect("Token not listed");
            let (base_lp, dfp2_lp) = self.get_lp_tokens(base_token);

            // Remove pair from database
            self.address_to_pair.remove(&base_token);
            self.address_to_pair.remove(&base_lp);
            self.address_to_pair.remove(&dfp2_lp);

            // Emit event
            Runtime::emit_event(TokenDeListed{base_token, component: *pair});
        }

        /// The `blacklist` function is intended to add a token to the blacklist, preventing it to be added to the
        /// DEX in the future. The function starts by checking that the token is not already present in the blacklist,
        /// then proceeds to add it. If the token is currently listed, it will be delisted.
        ///
        /// # Arguments
        ///
        /// * `token`: The `ResourceAddress` representing the token to be blacklisted.
        ///
        /// # Panics
        ///
        /// * The function will panic if the token is already found in the blacklist.
        ///
        /// # Events
        ///
        /// * Emits a `TokenBlacklisted` event after successfully blacklisting the token.
        pub fn blacklist(&mut self, token: ResourceAddress) {
            assert!(!self.blacklist.contains(&token), "Token already blacklisted");
            self.blacklist.insert(token);

            // Delist if currently listed
            if self.address_to_pair.get(&token).is_some() {
                self.delist(token);
            }

            // Emit event
            Runtime::emit_event(TokenBlacklisted{token});
        }

        /// The `deblacklist` function is responsible for removing a specific token from the blacklist, allowing
        /// it to be potentially added back to the exchange. The function first checks and asserts that the token
        /// is indeed present in the blacklist before removing it.
        ///
        /// # Parameters
        ///
        /// * `token`: A `ResourceAddress` signifying the target token to be removed from the blacklist.
        ///
        /// # Panics
        ///
        /// * The function will panic if the token is not initially found in the blacklist.
        ///
        /// # Emits
        ///
        /// * Emits a `TokenDeBlacklisted` event upon successful removal of the token from the blacklist.
        pub fn deblacklist(&mut self, token: ResourceAddress) {
            assert!(self.blacklist.contains(&token), "Token not blacklisted");
            self.blacklist.remove(&token);

            // Emit event
            Runtime::emit_event(TokenDeBlacklisted{token});
        }

        /// This function allows the DEX owner to update the Liquidity Pool (LP) token metadata.
        /// The `update_lp_metadata` function accesses the LP tokens corresponding to the provided PlazaPair at hand
        /// using its address. It then involves updating the associated metadata of the tokens with the provided
        /// key-value pair.
        ///
        /// # Arguments
        ///
        /// * `pair`: Specifies the targeted PlazaPair to be updated, defined as `Global<PlazaPair>`.
        /// * `key`: The key part of the metadata key-value pair that's to be updated, of type `String`
        /// * `value`: The new value that the updated metadata key should hold, represented as a `String`.
        ///
        /// # Panics
        ///
        /// This function will panic if the address of the provided PlazaPair does not exist within the stored pool
        /// of LP tokens.
        pub fn update_lp_metadata(&mut self, pair: Global<PlazaPair>, key: String, value: String) {
            let lp_tokens = self.pair_to_lps.get(&pair.address()).expect("Unknown pair");
            ResourceManager::from(lp_tokens.0).set_metadata(&key, value.to_owned());
            ResourceManager::from(lp_tokens.1).set_metadata(&key, value);
        }

        /// In case of a pool migration or delisting, the `withdraw_owned_liquidity` function provides an efficient
        /// mechanism for the withdrawal of DEX owned reserves from a specified pair. This function locates the
        /// pair's vaults in the `dex_reserves` hash map using the pair's address. It then proceeds to withdraw all
        /// the reserves from both vaults, returning them as a tuple of `Bucket` instances.
        ///
        /// # Arguments
        ///
        /// * `pair: Global<PlazaPair>` - The pair of reserves marked for withdrawal.
        ///
        /// # Returns
        ///
        /// * A tuple, `(Bucket, Bucket)`, containing the DEX-held LP tokens for the pair.
        ///
        /// # Panics
        ///
        /// The function will panic if it cannot locate the given pair's address inside the `dex_reserves` hash map.
        pub fn withdraw_owned_liquidity(&mut self, pair: Global<PlazaPair>) -> (Bucket, Bucket) {
            let mut vaults = self.dex_reserves.get_mut(&pair.address()).expect("Unknown pair");
            (vaults.0.take_all(), vaults.1.take_all())
        }

        /// Set Minimum DFP2: This method is used to update the minimum DFP2 amount needed to initiate a liquidity
        /// pair. The input argument 'min_dfp2' represents the updated minimum DFP2 value. Update the minimum DFP2
        /// amount required to create a pair
        pub fn set_min_dfp2(&mut self, min_dfp2: Decimal) {
            self.min_dfp2_liquidity = min_dfp2;
        }
   }
}