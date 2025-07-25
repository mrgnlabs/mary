use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use log::info;

use crate::config::Config;

pub struct MainService {
    stop: Arc<AtomicBool>,
    stats_interval_sec: u64,
}

impl MainService {
    pub fn new(config: Config, stop: Arc<AtomicBool>) -> Self {
        // Init cache
        // Init all services: geyser, liquidation, etc.

        MainService {
            stop,
            stats_interval_sec: config.stats_interval_sec,
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        info!("Starting the services...");

        info!("Entering the Main loop...");
        while !self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            info!("Stats: tbd ");
            thread::sleep(std::time::Duration::from_secs(self.stats_interval_sec));
        }
        info!("The Main loop stopped.");

        Ok(())
    }
}
