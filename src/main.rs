mod cache;
mod common;
mod comms;
mod config;
mod liquidation;
mod service;

use crate::comms::RpcCommsClient;
use crate::{config::Config, service::ServiceManager};
use env_logger::Builder;
use log::info;
use signal_hook::consts::{SIGINT, SIGTERM};
use std::{
    backtrace::Backtrace,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn main() -> anyhow::Result<()> {
    println!("Initializing...");

    // Panic hook
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Panic occurred: {:#?}", panic_info);

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Payload: {}", s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            eprintln!("Payload: {}", s);
        } else if let Some(err) = panic_info.payload().downcast_ref::<anyhow::Error>() {
            eprintln!("Payload: {:?}", err);
        } else {
            eprintln!("Payload: (unknown type)");
        }

        eprintln!("Exiting. Backtrace: {}", Backtrace::capture());

        std::process::exit(1);
    }));

    // Shutdown signal handlers
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, stop.clone()).unwrap();
    signal_hook::flag::register(SIGTERM, stop.clone()).unwrap();

    let stop_hook = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_hook.store(true, Ordering::SeqCst);
        println!("Received stop signal");
    })
    .expect("Error setting Ctrl-C handler");

    // Init Logger
    Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::new()?;
    info!("Configuration: {:?}", config);

    let service_manager: ServiceManager<RpcCommsClient> =
        ServiceManager::<RpcCommsClient>::new(config, stop.clone())?;
    service_manager.start()?;

    Ok(())
}
