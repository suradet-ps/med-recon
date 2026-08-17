//! Connection configuration for a HOSxP site.
//!
//! `HosxpConfig` carries only the fields needed to open a connection. Callers
//! obtain it from `recon-config` (encrypted at rest); the password is held
//! in a [`secrecy::SecretString`] and never logged.

use secrecy::SecretString;

/// HOSxP connection settings.
#[derive(Debug, Clone)]
pub struct HosxpConfig {
    /// Hostname or IP of the MySQL/MariaDB server.
    pub host: String,
    /// TCP port (default 3306).
    pub port: u16,
    /// HOSxP database name.
    pub database: String,
    /// Database user — recommended: a read-only role.
    pub user: String,
    /// Database password, kept in a secret wrapper.
    pub password: SecretString,
    /// How far back (days) medication/visit history is retrieved.
    pub history_days: u32,
}

impl HosxpConfig {
    /// Cutoff date (Christian era) for history queries, relative to `today`.
    pub fn history_cutoff(&self, today: chrono::NaiveDate) -> chrono::NaiveDate {
        today - chrono::Days::new(self.history_days as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn config() -> HosxpConfig {
        HosxpConfig {
            host: "localhost".into(),
            port: 3306,
            database: "hos".into(),
            user: "recon".into(),
            password: SecretString::from("s3cret"),
            history_days: 730,
        }
    }

    #[test]
    fn history_cutoff_subtracts_configured_days() {
        let c = config();
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        assert_eq!(
            c.history_cutoff(today),
            NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()
        );
    }
}
