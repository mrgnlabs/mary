use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug)]
pub struct Config {
    pub marginfi_program_id: Pubkey,
    pub stats_interval_sec: u64,
    pub rpc_url: String,
    pub geyser_endpoint: String,
    pub geyser_x_token: String,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let marginfi_program_id = Pubkey::from_str(
            &std::env::var("MARGINFI_PROGRAM_ID")
                .expect("MARGINFI_PROGRAM_ID environment variable is not set"),
        )
        .expect("Invalid MARGINFI_PROGRAM_ID Pubkey");

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
            marginfi_program_id,
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
            - marginfi_program_id: {} \n\
            - stats_interval_sec: {} \n\
            - geyser_endpoint: {}",
            self.marginfi_program_id, self.stats_interval_sec, self.geyser_endpoint
        )
    }
}

#[cfg(test)]
mod test_util {
    use std::env;

    pub const TEST_MARGINFI_PROGRAM_ID: &str = "11111111111111111111111111111111";
    pub const TEST_STATS_INTERVAL_SEC: &str = "60";
    pub const TEST_RPC_URL: &str = "http://dummy_rpc_url";
    pub const TEST_GEYSER_ENDPOINT: &str = "http://dummy_geyser_endpoint";
    pub const TEST_GEYSER_X_TOKEN: &str = "dummy_x_token";

    pub fn set_test_env() {
        env::set_var("MARGINFI_PROGRAM_ID", TEST_MARGINFI_PROGRAM_ID);
        env::set_var("STATS_INTERVAL_SEC", TEST_STATS_INTERVAL_SEC);
        env::set_var("RPC_URL", TEST_RPC_URL);
        env::set_var("GEYSER_ENDPOINT", TEST_GEYSER_ENDPOINT);
        env::set_var("GEYSER_X_TOKEN", TEST_GEYSER_X_TOKEN);
    }

    pub fn remove_env(key: &str) {
        env::remove_var(key);
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
}
