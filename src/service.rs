mod geyser_processor;
mod geyser_subscriber;
mod liquidation_service;

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use crate::{
    cache::{Cache, CacheLoader},
    service::geyser_subscriber::{GeyserMessage, GeyserSubscriber},
};
use crate::{comms::CommsClient, service::geyser_processor::GeyserProcessor};
use crate::{config::Config, service::liquidation_service::LiquidationService};
use anyhow::Result;
use bincode::deserialize;
use log::{error, info};
use solana_sdk::clock::Clock;
use solana_sdk::sysvar;

pub struct ServiceManager<T: CommsClient + 'static> {
    stop: Arc<AtomicBool>,
    stats_interval_sec: u64,
    cache: Arc<Cache>,
    cache_loader: CacheLoader<T>,
    geyser_subscriber: Arc<GeyserSubscriber>,
    geyser_processor: Arc<GeyserProcessor>,
    liquidation_service: Arc<LiquidationService<T>>,
}

impl<T: CommsClient + 'static> ServiceManager<T> {
    pub fn new(config: Config, stop: Arc<AtomicBool>) -> Result<Self> {
        // Fetch clock
        info!("Fetching the Solana Clock...");
        let comms_client = T::new(&config)?;
        let clock = fetch_clock(&comms_client)?;

        // Init cache
        info!("Initializing the Cache...");
        let cache = Arc::new(Cache::new(clock));

        info!("Initializing the CacheLoader...");
        let cache_loader = CacheLoader::new(&config, cache.clone())?;

        // Init Geyser services
        let (geyser_tx, geyser_rx) = crossbeam::channel::unbounded::<GeyserMessage>();

        info!("Initializing the GeyserSubscriber...");
        let geyser_subscriber =
            GeyserSubscriber::new(&config, stop.clone(), cache.clone(), geyser_tx)?;

        info!("Initializing the GeyserProcessor...");
        let geyser_processor = GeyserProcessor::new(stop.clone(), cache.clone(), geyser_rx);

        info!("Initializing the LiquidationService...");
        let liquidation_service: LiquidationService<T> =
            LiquidationService::new(stop.clone(), cache.clone(), comms_client)?;

        Ok(ServiceManager {
            stop,
            stats_interval_sec: config.stats_interval_sec,
            cache,
            cache_loader,
            geyser_subscriber: Arc::new(geyser_subscriber),
            geyser_processor: Arc::new(geyser_processor),
            liquidation_service: Arc::new(liquidation_service),
        })
    }

    pub fn start(&self) -> anyhow::Result<()> {
        info!("Starting services...");

        let geyser_processor = self.geyser_processor.clone();
        thread::spawn(move || {
            if let Err(e) = geyser_processor.run() {
                error!("GeyserProcessor failed! {:?}", e);
                panic!("Fatal error in GeyserProcessor!");
            }
        });

        let geyser_subscriber = self.geyser_subscriber.clone();
        thread::spawn(move || {
            if let Err(e) = geyser_subscriber.run() {
                error!("GeyserSubscriber failed! {:?}", e);
                panic!("Fatal error in GeyserSubscriber!");
            }
        });

        info!("Inflating the Cache...");
        self.cache_loader.load_cache()?;

        let liquidation_service = self.liquidation_service.clone();
        thread::spawn(move || {
            if let Err(e) = liquidation_service.run() {
                error!("LiquidationService failed! {:?}", e);
                panic!("Fatal error in LiquidationService!");
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
        let queue_depth = self.geyser_processor.queue_depth();
        info!(
            "Stats: [Latest Slot: {:?}; Geyser Queue Depth: {}]",
            clock.slot, queue_depth
        );
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
        let clock = generate_test_clock(1);

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
