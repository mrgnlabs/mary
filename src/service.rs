use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use crate::cache::Cache;
use crate::comms::CommsClient;
use crate::config::Config;
use anyhow::{anyhow, Result};
use bincode::deserialize;
use log::info;
use solana_sdk::clock::Clock;
use solana_sdk::sysvar::{self};

pub struct MainService<T: CommsClient> {
    stop: Arc<AtomicBool>,
    stats_interval_sec: u64,
    comms_client: T,
    cache: Arc<Cache>,
}

impl<T: CommsClient> MainService<T> {
    pub fn new(config: Config, stop: Arc<AtomicBool>) -> Result<Self> {
        let comms_client = T::new(&config)?;

        // Fetch clock
        info!("Fetching the Solana Clock...");
        let clock = fetch_clock(&comms_client)?;

        // Init cache
        info!("Initializing the Cache...");
        let cache = Arc::new(Cache::new(clock));

        // Init all services: geyser, liquidation, etc.

        Ok(MainService {
            stop,
            stats_interval_sec: config.stats_interval_sec,
            comms_client,
            cache,
        })
    }

    pub fn run(&self) -> anyhow::Result<()> {
        info!("Starting the services...");

        info!("Entering the Main loop...");
        while !self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            if let Err(err) = self.log_stats() {
                eprintln!("Error logging stats: {}", err);
            }
            thread::sleep(std::time::Duration::from_secs(self.stats_interval_sec));
        }
        info!("The Main loop stopped.");

        Ok(())
    }

    pub fn log_stats(&self) -> anyhow::Result<()> {
        let clock = self.cache.get_clock()?;
        info!("Stats: [Solana Clock: {:?}; ]", clock);
        Ok(())
    }
}

fn fetch_clock(rpc_client: &dyn CommsClient) -> anyhow::Result<Clock> {
    let clock_account = rpc_client.get_account(&sysvar::clock::id())?;
    let clock = deserialize(&clock_account.data)?;
    Ok(clock)
}
