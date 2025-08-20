use std::sync::Arc;

use log::debug;

use crate::{
    cache::{marginfi_accounts::CachedMarginfiAccount, Cache},
    liquidation::{CommsClient, LiquidationParams, LiquidationResult},
};

// Make sure to import or define the LiquidationStrategy trait
use crate::liquidation::LiquidationStrategy;

pub struct BasicLiquidationStrategy {}

impl LiquidationStrategy for BasicLiquidationStrategy {
    fn evaluate(
        &self,
        _account: &CachedMarginfiAccount,
    ) -> anyhow::Result<Option<LiquidationParams>> {
        debug!("Evaluating account {:?} for liquidation.", _account);
        Ok(Some(LiquidationParams {}))
    }

    fn liquidate<T: CommsClient>(
        &self,
        liquidation_params: LiquidationParams,
        _comms_client: &T,
    ) -> anyhow::Result<LiquidationResult> {
        debug!("Liquidating {:?}", liquidation_params);
        Ok(LiquidationResult {})
    }
}
