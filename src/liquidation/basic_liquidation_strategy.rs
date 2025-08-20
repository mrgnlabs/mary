use log::debug;

use crate::{
    cache::marginfi_accounts::CachedMarginfiAccount,
    liquidation::{CommsClient, LiquidationParams},
};

// Make sure to import or define the LiquidationStrategy trait
use crate::liquidation::LiquidationStrategy;

pub struct BasicLiquidationStrategy {}

impl LiquidationStrategy for BasicLiquidationStrategy {
    fn prepare(
        &self,
        _account: &CachedMarginfiAccount,
    ) -> anyhow::Result<Option<LiquidationParams>> {
        debug!("Preparing account {:?} for liquidation.", _account);
        Ok(Some(LiquidationParams {}))
    }

    fn liquidate<T: CommsClient>(
        &self,
        liquidation_params: LiquidationParams,
        _comms_client: &T,
    ) -> anyhow::Result<()> {
        debug!("Liquidating {:?}", liquidation_params);
        Ok(())
    }
}
