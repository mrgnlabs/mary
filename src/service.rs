mod geyser_subscriber;

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use crate::comms::CommsClient;
use crate::config::Config;
use crate::{cache::Cache, service::geyser_subscriber::GeyserSubscriber};
use anyhow::Result;
use bincode::deserialize;
use log::{error, info};
use solana_sdk::clock::Clock;
use solana_sdk::sysvar;

pub struct MainService<T: CommsClient> {
    stop: Arc<AtomicBool>,
    stats_interval_sec: u64,
    comms_client: T,
    cache: Arc<Cache>,
    geyser_subscriber: Arc<GeyserSubscriber>,
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

        // Init Geyser services
        info!("Initializing the Geyser Subscriber...");
        let geyser_subscriber = GeyserSubscriber::new(&config, stop.clone(), cache.clone())?;

        Ok(MainService {
            stop,
            stats_interval_sec: config.stats_interval_sec,
            comms_client,
            cache,
            geyser_subscriber: Arc::new(geyser_subscriber),
        })
    }

    pub fn run(&self) -> anyhow::Result<()> {
        info!("Starting services...");

        info!("Starting Geyser Subscriber...");
        let geyser_subscriber = self.geyser_subscriber.clone();
        thread::spawn(move || {
            if let Err(e) = geyser_subscriber.run() {
                error!("GeyserSubscriber failed! {:?}", e);
                panic!("Fatal error in GeyserSubscriber!");
            }
        });

        info!("Entering the Main loop.");
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

#[cfg(test)]
mod tests {
    use solana_sdk::account::Account;

    use super::*;
    use crate::cache::test_util::generate_test_clock;
    use crate::comms::test_util::MockedCommsClient;

    use std::collections::HashMap;

    #[test]
    fn test_fetch_clock() {
        let clock = generate_test_clock();

        let mut accounts = HashMap::new();
        accounts.insert(
            sysvar::clock::id(),
            Account {
                lamports: 0,
                data: bincode::serialize(&clock).unwrap(),
                owner: solana_sdk::pubkey::Pubkey::default(),
                executable: false,
                rent_epoch: 0,
            },
        );

        let mock_client = MockedCommsClient::with_accounts(accounts);
        let fetched_clock = fetch_clock(&mock_client).unwrap();
        assert_eq!(fetched_clock, clock);
    }
}
