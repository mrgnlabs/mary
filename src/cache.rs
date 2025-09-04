pub mod banks;
pub mod marginfi_accounts;

mod luts;
mod mints;
mod oracles;

use mints::MintsCache;
use oracles::OraclesCache;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, Result};
use log::{error, info, trace};
use marginfi::state::{marginfi_account::MarginfiAccount, marginfi_group::Bank};
use solana_program::clock::Clock;
use solana_sdk::{
    account::Account,
    address_lookup_table::{state::AddressLookupTable, AddressLookupTableAccount},
    pubkey::Pubkey,
};

use anchor_lang::AccountDeserialize;

use crate::{
    cache::{banks::BanksCache, luts::LutsCache, marginfi_accounts::MarginfiAccountsCache},
    common::{get_marginfi_message_type, MessageType},
    comms::CommsClient,
    config::Config,
};

// TODO: not completely sure that this trait is really needed.
pub trait CacheEntry {}

pub struct Cache {
    pub clock: RwLock<Clock>,
    pub marginfi_accounts: MarginfiAccountsCache,
    pub banks: BanksCache,
    pub mints: MintsCache,
    pub oracles: OraclesCache,
    pub luts: LutsCache,
}

impl Cache {
    pub fn new(clock: Clock) -> Self {
        Self {
            clock: RwLock::new(clock),
            marginfi_accounts: MarginfiAccountsCache::default(),
            banks: BanksCache::default(),
            mints: MintsCache::default(),
            oracles: OraclesCache::default(),
            luts: LutsCache::default(),
        }
    }

    pub fn update_clock(&self, clock: Clock) -> Result<()> {
        trace!("Updating Clock in cache: {:?}", clock);
        *self
            .clock
            .write()
            .map_err(|e| anyhow!("Failed to lock Clock for the update: {}", e))? = clock;
        Ok(())
    }

    pub fn get_clock(&self) -> Result<Clock> {
        Ok(self
            .clock
            .read()
            .map_err(|e| anyhow!("Failed to lock Clock for reading: {}", e))?
            .clone())
    }
}

//TODO: consider moving out to it's own module if it grows larger
pub struct CacheLoader<T: CommsClient> {
    program_id: Pubkey,
    lut_addresses: Vec<Pubkey>,
    cache: Arc<Cache>,
    comms_client: T,
}

impl<T: CommsClient> CacheLoader<T> {
    pub fn new(config: &Config, cache: Arc<Cache>) -> Result<Self> {
        let lut_addresses = config.lut_addresses.clone();
        let comms_client = T::new(config)?;
        Ok(Self {
            program_id: config.marginfi_program_id,
            lut_addresses,
            comms_client,
            cache,
        })
    }

    pub fn load_cache(&self) -> Result<()> {
        // Load Marginfi account and banks
        self.load_accounts()?;
        self.load_mints()?;
        self.load_oracles()?;
        self.load_luts()?;
        Ok(())
    }

    pub fn load_accounts(&self) -> Result<()> {
        info!("Loading Accounts for the Program id {}...", self.program_id);

        let slot = self.cache.get_clock()?.slot;

        let accounts = self.comms_client.get_program_accounts(&self.program_id)?;
        let mut marginfi_accounts_count = 0;
        let mut banks_count = 0;
        for (address, account) in accounts {
            match get_marginfi_message_type(&account.data) {
                Some(MessageType::MarginfiAccount) => {
                    let marginfi_account: MarginfiAccount =
                        MarginfiAccount::try_deserialize(&mut account.data.as_slice())?;
                    self.cache
                        .marginfi_accounts
                        .update(slot, address, &marginfi_account)?;
                    trace!("Added the Marginfi Account {:?} to cache.", address);
                    marginfi_accounts_count += 1;
                }
                Some(MessageType::Bank) => {
                    let bank: Bank = Bank::try_deserialize(&mut account.data.as_slice())?;
                    self.cache.banks.update(slot, address, &bank)?;
                    info!("Added the Bank {:?} to cache.", address);
                    banks_count += 1;
                }
                _ => {
                    // Not yet
                }
            }
        }

        info!(
            "Loaded {} Marginfi accounts and {} Banks.",
            marginfi_accounts_count, banks_count
        );

        Ok(())
    }

    pub fn load_mints(&self) -> Result<()> {
        info!("Loading Mints...");

        let mint_addresses = self.cache.banks.get_mints()?;

        let mut mints_counter = 0;
        for (address, mint) in self.comms_client.get_accounts(&mint_addresses)? {
            self.cache.mints.update(address, &mint)?;
            info!("Added the Mint {:?} to cache.", address);
            mints_counter += 1;
        }

        info!("Loaded {} Mints.", mints_counter);
        Ok(())
    }

    pub fn load_oracles(&self) -> Result<()> {
        info!("Loading Oracles...");

        let slot = self.cache.get_clock()?.slot;

        let oracles_data = self.cache.banks.get_oracles_data()?;
        let oracle_addresses: Vec<Pubkey> = oracles_data
            .iter()
            .flat_map(|oracle: &banks::CachedBankOracle| oracle.oracle_addresses.clone())
            .collect();

        let oracle_accounts: HashMap<Pubkey, Account> = self
            .comms_client
            .get_accounts(&oracle_addresses)?
            .into_iter()
            .collect();

        let mut oracle_counter = 0;
        for oracle_data in oracles_data {
            for oracle_address in oracle_data.oracle_addresses {
                match oracle_accounts.get(&oracle_address) {
                    Some(account) => {
                        if let Err(err) = self.cache.oracles.insert(
                            slot,
                            &oracle_address,
                            oracle_data.oracle_type,
                            account.clone(),
                        ) {
                            error!(
                                "Failed to add Oracle {:?} to cache: {}",
                                oracle_address, err
                            );
                        } else {
                            info!("Added the Oracle {:?} to cache.", oracle_address);
                            oracle_counter += 1;
                        }
                    }
                    None => {
                        error!("Failed to fetch the Oracle account {}", oracle_address);
                    }
                }
            }
        }

        info!("Loaded {} Oracles.", oracle_counter);
        Ok(())
    }

    pub fn load_luts(&self) -> Result<()> {
        if self.lut_addresses.is_empty() {
            info!("No LUT addresses provided, skipping LUT loading.");
            return Ok(());
        }

        info!("Loading Luts...");

        let lut_accounts = self.comms_client.get_accounts(&self.lut_addresses)?;

        let mut luts: Vec<AddressLookupTableAccount> = Vec::new();
        for (lut_address, lut_account) in lut_accounts {
            let lut = AddressLookupTable::deserialize(&lut_account.data)
                .map_err(|e| anyhow!("Failed to deserialize the {} LUT : {:?}", lut_address, e))?;
            luts.push(AddressLookupTableAccount {
                key: lut_address,
                addresses: lut.addresses.to_vec(),
            });
        }

        let luts_total = luts.len();
        self.cache.luts.populate(luts)?;

        info!("Loaded {} Luts.", luts_total);
        Ok(())
    }
}

#[cfg(test)]
pub mod test_util {
    use std::time::SystemTime;

    use solana_program::clock::Clock;
    use solana_sdk::clock::UnixTimestamp;

    use crate::cache::Cache;

    pub fn generate_test_clock(slot: u64) -> Clock {
        let current_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as UnixTimestamp;

        solana_program::clock::Clock {
            slot,
            epoch_start_timestamp: current_timestamp - 3600, // 1 hour ago
            epoch: 0,
            leader_schedule_epoch: 1,
            unix_timestamp: current_timestamp,
        }
    }

    pub fn create_dummy_cache() -> Cache {
        Cache::new(generate_test_clock(1))
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::generate_test_clock;
    use crate::cache::{banks::test_util::create_bank_with_oracles, test_util::create_dummy_cache};
    use crate::comms::test_util::MockedCommsClient;
    use crate::config::test_util::create_dummy_config;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::{account::Account, address_lookup_table::state::LookupTableMeta};
    use solana_sdk::{address_lookup_table::state::AddressLookupTable, signature::Keypair};
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_cache_new() {
        let clock = generate_test_clock(1);
        let cache = Cache::new(clock);
        assert_eq!(cache.get_clock().unwrap().slot, 1);
    }

    #[test]
    fn test_cache_update_clock() {
        let initial_clock = generate_test_clock(1);
        let cache = Cache::new(initial_clock);

        // Create a new clock with different values
        let mut updated_clock = generate_test_clock(2);
        updated_clock.epoch = 2;

        // Update the cache with the new clock
        cache.update_clock(updated_clock.clone()).unwrap();

        // Verify the cache now holds the updated clock
        let cached_clock = cache.get_clock().unwrap();
        assert_eq!(cached_clock.slot, 2);
        assert_eq!(cached_clock.epoch, 2);
        assert_eq!(cached_clock.unix_timestamp, updated_clock.unix_timestamp);
    }

    #[test]
    fn test_cache_loader_new() {
        // Prepare dummy config and cache
        let config = create_dummy_config();
        let cache = Arc::new(create_dummy_cache());

        // Try to create a CacheLoader using the mocked comms client
        let loader = CacheLoader::<MockedCommsClient>::new(&config, cache.clone());
        assert!(loader.is_ok());
        let loader = loader.unwrap();
        assert_eq!(loader.program_id, config.marginfi_program_id);
    }

    //TODO: add the CacheLoader tests after figuring out how to serialize MarginfiAccount.

    #[test]
    fn test_cache_loader_load_mints() {
        // Prepare dummy config and cache
        let config = create_dummy_config();
        let cache = Arc::new(create_dummy_cache());

        // Insert a dummy bank with a mint address into the cache
        let mint_pubkey = Pubkey::new_unique();
        let dummy_bank = create_bank_with_oracles(vec![mint_pubkey]);
        cache
            .banks
            .update(1, Pubkey::new_unique(), &dummy_bank)
            .unwrap();

        // Prepare a mocked comms client that returns a dummy mint account
        let pubkey = Pubkey::new_unique();
        let account = Account {
            lamports: 1,
            data: vec![0u8; 82], // dummy mint data
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };
        let mut accounts = HashMap::new();
        accounts.insert(pubkey, account);
        let mocked_client = MockedCommsClient::with_accounts(accounts);

        // Create the loader with the mocked client
        let loader = CacheLoader {
            program_id: config.marginfi_program_id,
            lut_addresses: vec![],
            comms_client: mocked_client,
            cache: cache.clone(),
        };

        // Call load_mints and check that the mint was added to the cache
        let result = loader.load_mints();
        assert!(result.is_ok());

        // The mint should now be present in the cache
        let mints = &cache.mints;
        assert!(mints.get(&mint_pubkey).is_ok());
    }

    #[test]
    fn test_cache_loader_load_oracles() {
        // Prepare dummy config and cache
        let config = create_dummy_config();
        let cache = Arc::new(create_dummy_cache());

        // Create dummy oracle addresses and a dummy CachedBank with oracles
        let oracle_pubkey1 = Pubkey::new_unique();
        let oracle_pubkey2 = Pubkey::new_unique();
        let dummy_bank = create_bank_with_oracles(vec![]);
        let cached_bank = create_bank_with_oracles(vec![oracle_pubkey1, oracle_pubkey2]);

        cache
            .banks
            .update(1, Pubkey::new_unique(), &dummy_bank)
            .unwrap();
        cache
            .banks
            .update(1, Pubkey::new_unique(), &cached_bank)
            .unwrap();

        // Prepare dummy oracle accounts
        let account1 = Account {
            lamports: 1,
            data: vec![0u8; 100],
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };
        let account2 = Account {
            lamports: 2,
            data: vec![1u8; 100],
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };
        let mut accounts = HashMap::new();
        accounts.insert(oracle_pubkey1, account1.clone());
        accounts.insert(oracle_pubkey2, account2.clone());

        let mocked_client = MockedCommsClient::with_accounts(accounts);

        // Create the loader with the mocked client
        let loader = CacheLoader {
            program_id: config.marginfi_program_id,
            lut_addresses: vec![],
            comms_client: mocked_client,
            cache: cache.clone(),
        };

        // Call load_oracles and check that the oracles were added to the cache
        let result = loader.load_oracles();
        assert!(result.is_ok());

        // The oracles should now be present in the cache
        let oracles_cache = &cache.oracles;
        assert!(oracles_cache.get(&oracle_pubkey1).is_ok());
        assert!(oracles_cache.get(&oracle_pubkey2).is_ok());
    }

    #[test]
    fn test_cache_loader_load_luts() {
        let mut config = create_dummy_config();
        // Prepare dummy config and cache
        let lut_address = Pubkey::new_unique();
        config.lut_addresses.push(lut_address);
        let cache = Arc::new(create_dummy_cache());

        // Create dummy LUT data
        let dummy_addresses: Vec<Pubkey> = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let lut = AddressLookupTable {
            meta: LookupTableMeta::default(),
            addresses: dummy_addresses.clone().try_into().unwrap_or_default(),
        };
        let lut_account_data = AddressLookupTable::serialize_for_tests(lut.clone()).unwrap();
        let lut_account = Account {
            lamports: 1,
            data: lut_account_data,
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };

        let mut accounts = HashMap::new();
        accounts.insert(lut_address, lut_account);

        let mocked_client = MockedCommsClient::with_accounts(accounts);

        // Create the loader with the mocked client
        let loader = CacheLoader {
            program_id: config.marginfi_program_id,
            lut_addresses: config.lut_addresses.clone(),
            comms_client: mocked_client,
            cache: cache.clone(),
        };

        // Call load_luts and check that the LUTs were added to the cache
        let result = loader.load_luts();
        assert!(result.is_ok());

        // The LUT should now be present in the cache
        let luts_cache = &cache.luts;
        let luts = luts_cache.get_all().unwrap();
        assert!(!luts.is_empty());
        assert!(luts.iter().any(|lut| lut.key == lut_address));
    }
}
