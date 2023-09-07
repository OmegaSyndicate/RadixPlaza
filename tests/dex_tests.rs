// mod plazadex_tests {
//     use std::io::Write;
//     use std::fs::{File, create_dir_all};
//     use scrypto_unit::prelude::*;
//     use radix_engine_interface::radix_engine_common::dec;
//     use radix_engine::transaction::TransactionReceipt;
//     use radix_engine_interface::blueprints::resource::OwnerRole;
//     use test_engine::env_args;
//     use test_engine::environment::{Environment};
//     use test_engine::test_engine::TestEngine;
//     use test_engine::receipt_traits::{GetReturn, Outcome};

//     fn save_receipt_to_file(filename: &str, receipt: &TransactionReceipt) {
//         create_dir_all("debug").expect("Unable to create 'debug' directory");
//         let filepath = format!("debug/{}", filename);
//         let mut file = File::create(&filepath).expect("Unable to create file");
//         let receipt_string = format!("{:?}\n", receipt);
//         file.write_all(receipt_string.as_bytes())
//             .expect("Unable to write to file");
//     }

//     fn initialize() -> TestEngine {
//         let mut test_engine = TestEngine::new();
//         // let config = PairConfig {
//         //     k_in: dec!("0.4"),
//         //     k_out: dec!("1"),
//         //     fee: dec!("0.003"),
//         // };
//         test_engine.new_token("dfp2", dec!(1_000_000));
//         test_engine.new_token("toka", dec!(1_000_000));
//         test_engine.new_token("tokb", dec!(1_000_000));
//         test_engine.new_package("defiplaza package", "./");
//         test_engine.new_component(
//             "plazadex",
//             "PlazaDex",
//             "instantiate_dex",
//             env_args!(
//                 Environment::Resource("dfp2")
//             ),
//         );
//         test_engine.call_method(
//             "create_pair",
//             env_args!(
//                 Environment::Resource("toka"),
//                 Environment::FungibleBucket("dfp2", dec!(1000)),
//                 dec!(1)
//             ),
//         );
//         test_engine.call_method(
//             "create_pair",
//             env_args!(
//                 Environment::Resource("tokb"),
//                 Environment::FungibleBucket("dfp2", dec!(1000)),
//                 dec!(1)
//             ),
//         );        

//         test_engine
//     }

//     #[test]
//     fn instantiates()  {
//         let mut _test_engine = initialize();
//     }
// }