mod basic_liquidation_strategy;
use basic_liquidation_strategy::BasicLiquidationStrategy;

//TODO: totally in draft! Sketched the enum and trait for an inspiration

pub enum LiqudationStrategyType {
    Basic(BasicLiquidationStrategy),
    Kamino,
    Arena,
}

pub trait LiqudationStrategy {
    // fn evaluate(&self, account: Account) -> anyhow::Result<LiqudationParams>;
    // fn liquidate(&self, liquidation_params: LiquidationParams, comms_client: & CommsClient) -> anyhow::Result<LiqudationResult>;
}

// TODO: fn choose_liquidation_strategy_type(account: Account, cache: &Arc<Cache>) -> impl LiqudationStrategy
