use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Debug)]
pub struct Config {
    pub marginfi_program_id: Pubkey,
    pub stats_interval_sec: u64,
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

        Ok(Config {
            marginfi_program_id,
            stats_interval_sec,
        })
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Config: \n\
            - marginfi_program_id: {}",
            self.marginfi_program_id
        )
    }
}
#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    fn set_env(key: &str, value: &str) {
        env::set_var(key, value);
    }

    fn remove_env(key: &str) {
        env::remove_var(key);
    }

    #[test]
    fn test_config_new_success() {
        let pubkey = "11111111111111111111111111111111";
        set_env("MARGINFI_PROGRAM_ID", pubkey);
        set_env("STATS_INTERVAL_SEC", "60");

        let config = Config::new().unwrap();
        assert_eq!(
            config.marginfi_program_id,
            Pubkey::from_str(pubkey).unwrap()
        );
        assert_eq!(config.stats_interval_sec, 60);

        remove_env("MARGINFI_PROGRAM_ID");
        remove_env("STATS_INTERVAL_SEC");
    }

    #[test]
    #[should_panic(expected = "MARGINFI_PROGRAM_ID environment variable is not set")]
    fn test_config_missing_marginfi_program_id() {
        remove_env("MARGINFI_PROGRAM_ID");
        set_env("STATS_INTERVAL_SEC", "60");
        let _ = Config::new();
    }

    #[test]
    #[should_panic(expected = "Invalid MARGINFI_PROGRAM_ID Pubkey")]
    fn test_config_invalid_marginfi_program_id() {
        set_env("MARGINFI_PROGRAM_ID", "invalid_pubkey");
        set_env("STATS_INTERVAL_SEC", "60");
        let _ = Config::new();
    }

    #[test]
    #[should_panic(expected = "STATS_INTERVAL_SEC environment variable is not set")]
    fn test_config_missing_stats_interval_sec() {
        set_env("MARGINFI_PROGRAM_ID", "11111111111111111111111111111111");
        remove_env("STATS_INTERVAL_SEC");
        let _ = Config::new();
    }

    #[test]
    #[should_panic(expected = "Invalid STATS_INTERVAL_SEC value, must be a number")]
    fn test_config_invalid_stats_interval_sec() {
        set_env("MARGINFI_PROGRAM_ID", "11111111111111111111111111111111");
        set_env("STATS_INTERVAL_SEC", "not_a_number");
        let _ = Config::new();
    }

    #[test]
    fn test_config_display() {
        let pubkey = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let config = Config {
            marginfi_program_id: pubkey,
            stats_interval_sec: 42,
        };
        let display = format!("{}", config);
        assert!(display.contains("marginfi_program_id: 11111111111111111111111111111111"));
    }
}
