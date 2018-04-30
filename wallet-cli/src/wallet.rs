use wallet_crypto::{wallet, hdwallet, bip44};
use wallet_crypto::util::hex::{encode, decode};
use wallet_crypto::util::base58;
use command::{HasCommand};
use clap::{ArgMatches, Arg, SubCommand, App};
use config::{Config};
use account::{Account};
use storage::{Storage, StorageConfig, pack_blobs, pack, PackParameters};
use rand;

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet(wallet::Wallet);
impl Wallet {
    fn generate() -> Self {
        let mut bytes = [0u8;hdwallet::SEED_SIZE];
        for byte in bytes.iter_mut() {
            *byte = rand::random();
        }
        let seed = hdwallet::Seed::from_bytes(bytes);
        Wallet(wallet::Wallet::new_from_seed(&seed))
    }
}

impl HasCommand for Wallet {
    type Output = Option<Config>;

    fn clap_options<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("wallet")
            .about("wallet management")
            .subcommand(SubCommand::with_name("generate")
                .about("generate a new wallet")
            )
            .subcommand(SubCommand::with_name("address")
                .about("create an address with the given options")
                .arg(Arg::with_name("is_internal").long("internal").help("to generate an internal address (see BIP44)"))
                .arg(Arg::with_name("account").help("account to generate an address in").index(1).required(true))
                .arg(Arg::with_name("indices")
                    .help("list of indices for the addresses to create")
                    .multiple(true)
                )
            )
            .subcommand(SubCommand::with_name("debug-index")
                .about("internal debug command")
                .arg(Arg::with_name("packhash").help("pack to query").index(1))
            )
            .subcommand(SubCommand::with_name("pack")
                .about("internal pack command")
            )
    }
    fn run(config: Config, args: &ArgMatches) -> Self::Output {
        let mut cfg = config;
        match args.subcommand() {
            ("generate", _) => {
                // expect no existing wallet
                assert!(cfg.wallet.is_none());
                cfg.wallet = Some(Wallet::generate());
                let store_config = StorageConfig::new(&cfg.storage, &cfg.network_type);
                let storage = Storage::init(&store_config).unwrap();
                Some(cfg) // we need to update the config's wallet
            },
            ("debug-index", opts) => {
                let store_config = StorageConfig::new(&cfg.storage, &cfg.network_type);
                let storage = Storage::init(&store_config).unwrap();
                match opts {
                    None    => {
                        let vs = store_config.list_indexes();
                        for &v in vs.iter() {
                            println!("{}", encode(&v));
                        }
                    },
                    Some(opts) => {
                        let packrefhex = opts.value_of("packhash")
                            .and_then(|s| Some(s.to_string()))
                            .unwrap();
                        let mut packref = [0u8;32];
                        packref.clone_from_slice(&decode(&packrefhex).unwrap()[..]);
                        let (fanout, refs) = pack::dump_index(&store_config, &packref).unwrap();
                        for r in refs.iter() {
                            println!("{}", encode(r));
                        }
                    }
                }
                Some(cfg)
            },
            ("pack", _) => {
                let store_config = StorageConfig::new(&cfg.storage, &cfg.network_type);
                let mut storage = Storage::init(&store_config).unwrap();
                let pack_params = PackParameters {
                    limit_nb_blobs: None,
                    limit_size: None,
                    delete_blobs_after_pack: false,
                };
                let packhash = pack_blobs(&mut storage, &pack_params);
                println!("pack created: {}", encode(&packhash));
                Some(cfg)
            }
            ("address", Some(opts)) => {
                // expect existing wallet
                assert!(cfg.wallet.is_some());
                match &cfg.wallet {
                    &None => panic!("No wallet created, see `wallet generate` command"),
                    &Some(ref wallet) => {
                        let addr_type = if opts.is_present("is_internal") {
                            bip44::AddrType::Internal
                        } else {
                            bip44::AddrType::External
                        };
                        let account_name = opts.value_of("account")
                            .and_then(|s| Some(Account::new(s.to_string())))
                            .unwrap();
                        let account = match cfg.find_account(&account_name) {
                            None => panic!("no account {:?}", account_name),
                            Some(r) => r,
                        };
                        let indices = values_t!(opts.values_of("indices"), u32).unwrap_or_else(|_| vec![0]);

                        let addresses = wallet.0.gen_addresses(account, addr_type, indices);
                        for addr in addresses {
                            println!("{}", base58::encode(&addr.to_bytes()));
                        };
                        None // we don't need to update the wallet
                    }
                }
            },
            _ => {
                println!("{}", args.usage());
                ::std::process::exit(1);
            },
        }
    }
}
