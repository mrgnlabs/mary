mod basic_liquidation_strategy;
use basic_liquidation_strategy::BasicLiquidationStrategy;
use std::sync::Arc;

use crate::{
    cache::{marginfi_accounts::CachedMarginfiAccount, Cache},
    comms::CommsClient,
};

pub trait LiquidationStrategy {
    fn prepare(&self, account: &CachedMarginfiAccount)
        -> anyhow::Result<Option<LiquidationParams>>;
    fn liquidate<T: CommsClient>(
        &self,
        liquidation_params: LiquidationParams,
        comms_client: &T,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct LiquidationParams {}

// TODO: create static reusable strategy objects instead of initializing them each time
pub fn choose_liquidation_strategy(
    _account: &CachedMarginfiAccount,
    _cache: &Arc<Cache>,
) -> anyhow::Result<impl LiquidationStrategy> {
    // For now, we'll just use the basic strategy
    Ok(BasicLiquidationStrategy {})
}
