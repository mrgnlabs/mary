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
        /*
        1. Calc total account's  assets amount in USD.
        2. Calc total account's liab amount in USD.
        3. Confirm that the account is liquidable.
        4. Select collat and liab banks.
        5. Calc the collat withdraw and liab repay amounts.
        6. Confirm that the liquidation profit in USD > the configured min liquidation profit.
        7. Create the LiquidationParams object.
        */
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
