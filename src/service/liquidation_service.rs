use crate::comms::CommsClient;

// init
pub struct LiquidationService {}

impl LiquidationService {
    pub fn new() -> Self {
        LiquidationService {}
    }

    pub fn run<T: CommsClient>(&self, comms_client: &T) -> anyhow::Result<()> {
        // 1. Loop thru all accounts sorted by health
        // 2. For each account
        // 2.1 choose liquidation strategy
        // 2.2 liquidation_strategy.evaluate(account)
        // 2.3 liquidation_strategy.liquidate(liquidation_params, comms_client)
        Ok(())
    }
}
