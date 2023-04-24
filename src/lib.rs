use scrypto::prelude::*;

#[blueprint]
mod plazapair {
    struct PlazaPair {
        // Define what resources and data will be managed by a liquidity pair
        p0: Decimal,
        base_target: Decimal,
        quote_target: Decimal,
        base_vault: Vault,
        quote_vault: Vault,
        k_in: Decimal,
        k_out: Decimal,
        base_lp: ResourceAddress,
        quote_lp: ResourceAddress,
        lp_badge: Vault,
    }

    impl PlazaPair {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_pair(initial_base: Bucket, initial_quote: Bucket, price: Decimal)
                -> (ComponentAddress, Bucket, Bucket) {
            // Create internal admin badge
            let lp_badge: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "admin badge")
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(1);
            
            // Create a new token called "HelloToken," with a fixed supply of 1000, and put that supply into a bucket
            let base_lp_bucket: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "PlazaPair Base LP")
                .metadata("symbol", "PLAZALP")
                .mintable(rule!(require(lp_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lp_badge.resource_address())), LOCKED)
                .mint_initial_supply(initial_base.amount());

            let quote_lp_bucket: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "PlazaPair Quote LP")
                .metadata("symbol", "PLAZALP")
                .mintable(rule!(require(lp_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lp_badge.resource_address())), LOCKED)
                .mint_initial_supply(initial_quote.amount());

            // Instantiate a Hello component, populating its vault with our supply of 1000 HelloToken
            let pair = Self {
                p0: price,
                base_target: initial_base.amount(),
                quote_target: initial_quote.amount(),
                base_vault: Vault::with_bucket(initial_base),
                quote_vault: Vault::with_bucket(initial_quote),
                k_in: dec!("0.4"),
                k_out: dec!("1.0"),
                base_lp: base_lp_bucket.resource_address(),
                quote_lp: quote_lp_bucket.resource_address(),
                lp_badge: Vault::with_bucket(lp_badge), 
            }
                .instantiate()
                .globalize();
            
            (pair, base_lp_bucket, quote_lp_bucket)
        }

        // // This is a method, because it needs a reference to self.  Methods can only be called on components
        // pub fn free_token(&mut self) -> Bucket {
        //     info!("My balance is: {} HelloToken. Now giving away a token!", self.sample_vault.amount());
        //     // If the semi-colon is omitted on the last line, the last value seen is automatically returned
        //     // In this case, a bucket containing 1 HelloToken is returned
        //     self.sample_vault.take(1)
        // }
    }
}