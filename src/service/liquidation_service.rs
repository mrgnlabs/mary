use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::Result;

use log::{error, info};
use solana_sdk::pubkey::Pubkey;

use crate::{
    cache::Cache,
    comms::CommsClient,
    liquidation::{choose_liquidation_strategy, LiquidationStrategy},
};

pub struct LiquidationService<T>
where
    T: CommsClient + 'static,
{
    stop: Arc<AtomicBool>,
    cache: Arc<Cache>,
    comms_client: T,
}

impl<T: CommsClient> LiquidationService<T> {
    pub fn new(stop: Arc<AtomicBool>, cache: Arc<Cache>, comms_client: T) -> Result<Self> {
        Ok(Self {
            stop,
            cache,
            comms_client,
        })
    }

    pub fn run(&self) -> anyhow::Result<()> {
        info!("Entering the LiquidationService loop.");
        while !self.stop.load(Ordering::Relaxed) {
            info!("Starting the Liquidation cycle...");
            match self.cache.marginfi_accounts.get_accounts_with_health() {
                Ok(accounts_by_health) => {
                    let sorted_accounts = sort_accounts_by_health(&accounts_by_health);
                    for account_address in sorted_accounts {
                        if let Err(err) = self.process_account(account_address) {
                            error!(
                                "Failed to process the Marginfi account {}: {}",
                                account_address, err
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get the Marginfiaccounts with health map: {}", e);
                    continue;
                }
            };
            info!("Liquidation cycle is completed.");
            // Temporary hack to avoid busy spin
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        info!("The LiquidationService loop is stopped.");
        Ok(())
    }

    fn process_account(&self, address: Pubkey) -> Result<()> {
        let account = self.cache.marginfi_accounts.get(&address)?;
        let liquidation_strategy = choose_liquidation_strategy(&account, &self.cache)?;
        if let Some(lq_params) = liquidation_strategy.prepare(&account)? {
            liquidation_strategy.liquidate(lq_params, &self.comms_client)?;
        }
        Ok(())
    }
}

fn sort_accounts_by_health(accounts: &HashMap<Pubkey, u64>) -> Vec<Pubkey> {
    let mut sorted: Vec<(Pubkey, u64)> = accounts.iter().map(|(&k, &v)| (k, v)).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().map(|(k, _)| k).collect()
}
