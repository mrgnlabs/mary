use solana_program::pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};
use std::str::FromStr;

pub struct Config {
    pub wallet: Keypair,
    pub marginfi_program_id: Pubkey,
    pub lut_addresses: Vec<Pubkey>,
    pub stats_interval_sec: u64,
    pub rpc_url: String,
    pub geyser_endpoint: String,
    pub geyser_x_token: String,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let wallet_str = std::env::var("WALLET").expect("WALLET environment variable is not set");
        let wallet_bytes: Vec<u8> = serde_json::from_str(&wallet_str)
            .map_err(|e| anyhow::anyhow!("Invalid WALLET format (JSON): {}", e))?;
        let wallet = Keypair::from_bytes(&wallet_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid WALLET format (Keypair bytes): {}", e))?;

        let marginfi_program_id = Pubkey::from_str(
            &std::env::var("MARGINFI_PROGRAM_ID")
                .expect("MARGINFI_PROGRAM_ID environment variable is not set"),
        )
        .expect("Invalid MARGINFI_PROGRAM_ID Pubkey");

        let lut_addresses: Vec<Pubkey> = std::env::var("LUT_ADDRESSES")
            .expect("LUT_ADDRESSES environment variable is not set")
            .split(',')
            .map(|s| {
                Pubkey::from_str(s.trim())
                    .map_err(|_| anyhow::anyhow!("Invalid LUT_ADDRESSES Pubkey: {}", s.trim()))
            })
            .collect::<Result<_, _>>()?;

        let stats_interval_sec = std::env::var("STATS_INTERVAL_SEC")
            .expect("STATS_INTERVAL_SEC environment variable is not set")
            .parse::<u64>()
            .expect("Invalid STATS_INTERVAL_SEC value, must be a number");

        let rpc_url = std::env::var("RPC_URL").expect("RPC_URL environment variable is not set");

        let geyser_endpoint = std::env::var("GEYSER_ENDPOINT")
            .expect("GEYSER_ENDPOINT environment variable is not set");
        let geyser_x_token = std::env::var("GEYSER_X_TOKEN")
            .expect("GEYSER_X_TOKEN environment variable is not set");

        Ok(Config {
            wallet,
            marginfi_program_id,
            lut_addresses,
            stats_interval_sec,
            rpc_url,
            geyser_endpoint,
            geyser_x_token,
        })
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Config: \n\
            - wallet: {} \n\
            - marginfi_program_id: {} \n\
            - lut_addresses: [{}] \n\
            - stats_interval_sec: {} \n\
            - geyser_endpoint: {}",
            self.wallet.pubkey(),
            self.marginfi_program_id,
            self.lut_addresses
                .iter()
                .map(|addr| addr.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.stats_interval_sec,
            self.geyser_endpoint
        )
    }
}

#[cfg(test)]
pub mod test_util {
    use std::env;

    use solana_sdk::{pubkey::Pubkey, signature::Keypair};

    use crate::config::Config;

    pub const TEST_MARGINFI_PROGRAM_ID: &str = "11111111111111111111111111111111";
    pub const TEST_STATS_INTERVAL_SEC: &str = "60";
    pub const TEST_RPC_URL: &str = "http://dummy_rpc_url";
    pub const TEST_GEYSER_ENDPOINT: &str = "http://dummy_geyser_endpoint";
    pub const TEST_GEYSER_X_TOKEN: &str = "dummy_x_token";

    pub fn set_test_env() {
        env::set_var(
            "WALLET",
            serde_json::to_string(&Keypair::new().to_bytes().to_vec()).unwrap(),
        );
        env::set_var("MARGINFI_PROGRAM_ID", TEST_MARGINFI_PROGRAM_ID);
        env::set_var(
            "LUT_ADDRESSES",
            &format!(
                "{},{}",
                solana_program::pubkey::Pubkey::new_unique(),
                solana_program::pubkey::Pubkey::new_unique()
            ),
        );
        env::set_var("STATS_INTERVAL_SEC", TEST_STATS_INTERVAL_SEC);
        env::set_var("RPC_URL", TEST_RPC_URL);
        env::set_var("GEYSER_ENDPOINT", TEST_GEYSER_ENDPOINT);
        env::set_var("GEYSER_X_TOKEN", TEST_GEYSER_X_TOKEN);
    }

    pub fn remove_env(key: &str) {
        env::remove_var(key);
    }

    pub fn create_dummy_config() -> Config {
        let wallet = Keypair::new();
        let marginfi_program_id = Pubkey::new_unique();
        let lut_addresses = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let stats_interval_sec = 60;
        let rpc_url = "http://dummy_rpc_url".into();
        let geyser_endpoint = "http://dummy_geyser_endpoint".into();
        let geyser_x_token = "dummy_x_token".into();

        Config {
            wallet,
            marginfi_program_id,
            lut_addresses,
            stats_interval_sec,
            rpc_url,
            geyser_endpoint,
            geyser_x_token,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::test_util::{
        remove_env, set_test_env, TEST_GEYSER_ENDPOINT, TEST_GEYSER_X_TOKEN,
        TEST_MARGINFI_PROGRAM_ID, TEST_RPC_URL, TEST_STATS_INTERVAL_SEC,
    };

    use serial_test::serial;
    use std::env;

    use super::*;

    #[test]
    #[serial]
    fn test_config_new_success() {
        set_test_env();

        let config = Config::new().unwrap();
        assert_eq!(
            config.marginfi_program_id.to_string(),
            TEST_MARGINFI_PROGRAM_ID
        );
        assert_eq!(
            config.stats_interval_sec,
            TEST_STATS_INTERVAL_SEC.parse::<u64>().unwrap()
        );
        assert_eq!(config.rpc_url, TEST_RPC_URL);
        assert_eq!(config.geyser_endpoint, TEST_GEYSER_ENDPOINT);
        assert_eq!(config.geyser_x_token, TEST_GEYSER_X_TOKEN);
    }

    #[test]
    #[serial]
    #[should_panic(expected = "MARGINFI_PROGRAM_ID environment variable is not set")]
    fn test_config_missing_marginfi_program_id() {
        set_test_env();
        remove_env("MARGINFI_PROGRAM_ID");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid MARGINFI_PROGRAM_ID Pubkey")]
    fn test_config_invalid_marginfi_program_id() {
        set_test_env();
        env::set_var("MARGINFI_PROGRAM_ID", "invalid_pubkey");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "STATS_INTERVAL_SEC environment variable is not set")]
    fn test_config_missing_stats_interval_sec() {
        set_test_env();
        remove_env("STATS_INTERVAL_SEC");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid STATS_INTERVAL_SEC value, must be a number")]
    fn test_config_invalid_stats_interval_sec() {
        set_test_env();
        env::set_var("STATS_INTERVAL_SEC", "not_a_number");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "GEYSER_ENDPOINT environment variable is not set")]
    fn test_config_missing_geyser_endpoint() {
        set_test_env();
        remove_env("GEYSER_ENDPOINT");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "GEYSER_X_TOKEN environment variable is not set")]
    fn test_config_missing_geyser_x_token() {
        set_test_env();
        remove_env("GEYSER_X_TOKEN");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    fn test_config_display() {
        super::test_util::set_test_env();
        let config = Config::new().unwrap();
        let display = format!("{}", config);
        assert!(display.contains(&format!(
            "marginfi_program_id: {}",
            super::test_util::TEST_MARGINFI_PROGRAM_ID
        )));
        assert!(display.contains(&format!(
            "stats_interval_sec: {}",
            super::test_util::TEST_STATS_INTERVAL_SEC
        )));
    }

    #[test]
    #[serial]
    fn test_config_lut_addresses_parsing() {
        super::test_util::set_test_env();

        let pk1 = Pubkey::new_unique();
        let pk2 = Pubkey::new_unique();
        // Set LUT_ADDRESSES to two valid pubkeys
        let lut_addresses = format!("{},{}", pk1, pk2);
        std::env::set_var("LUT_ADDRESSES", lut_addresses);

        let config = Config::new().unwrap();
        assert_eq!(config.lut_addresses.len(), 2);
        assert_eq!(config.lut_addresses[0].to_string(), pk1.to_string());
        assert_eq!(config.lut_addresses[1].to_string(), pk2.to_string());
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid LUT_ADDRESSES Pubkey:")]
    fn test_config_lut_addresses_empty() {
        super::test_util::set_test_env();
        std::env::set_var("LUT_ADDRESSES", "");
        let _ = Config::new().unwrap();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "LUT_ADDRESSES environment variable is not set")]
    fn test_config_missing_lut_addresses() {
        super::test_util::set_test_env();
        super::test_util::remove_env("LUT_ADDRESSES");
        let _ = Config::new();
    }

    #[test]
    #[serial]
    #[should_panic(expected = "Invalid LUT_ADDRESSES Pubkey:")]
    fn test_config_lut_addresses_with_invalid_pubkey() {
        super::test_util::set_test_env();
        // One valid, one invalid pubkey
        std::env::set_var(
            "LUT_ADDRESSES",
            "11111111111111111111111111111111,invalid_pubkey",
        );
        let config = Config::new().unwrap();
        // Only the valid pubkey should be parsed
        assert_eq!(config.lut_addresses.len(), 1);
        assert_eq!(
            config.lut_addresses[0].to_string(),
            "11111111111111111111111111111111"
        );
    }
}
