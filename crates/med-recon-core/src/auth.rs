//! HOSxP officer authentication types and password verification.
//!
//! Med Recon signs operators in with their HOSxP **`officer`** account. The
//! stored MD5 digest never crosses the Tauri IPC boundary; only the
//! sanitized [`SessionUser`] does. The MD5 assumption and its caveats are
//! documented in `docs/auth.md`.

use std::fmt;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

/// One `officer` row as read from HOSxP. Internal to the backend crates;
/// the password hash is never serialized.
#[derive(Clone, PartialEq, Eq)]
pub struct OfficerAccount {
    /// `officer.officer_id`.
    pub officer_id: i64,
    /// `officer.officer_login_name` - the username.
    pub login_name: String,
    /// `officer.officer_login_password_md5` - 32 hex characters.
    pub password_md5: String,
    /// `officer.officer_active`; only `"Y"` is accepted.
    pub active: String,
    /// `officer.officer_authentication_type_id`; `NULL` was treated as 1
    /// (local database) in the source query.
    pub auth_type_id: i64,
    /// `officer.officer_doctor_code`, when present.
    pub doctor_code: Option<String>,
    /// Display name assembled from the officer name fields.
    pub display_name: String,
}

impl fmt::Debug for OfficerAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OfficerAccount")
            .field("officer_id", &self.officer_id)
            .field("login_name", &self.login_name)
            .field("password_md5", &"<redacted>")
            .field("active", &self.active)
            .field("auth_type_id", &self.auth_type_id)
            .field("doctor_code", &self.doctor_code)
            .field("display_name", &self.display_name)
            .finish()
    }
}

/// The sanitized user returned to the frontend after a successful login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    /// `officer.officer_id`.
    pub officer_id: i64,
    /// HOSxP login name.
    pub login_name: String,
    /// Display name for the top bar; falls back to the login name.
    pub display_name: String,
    /// `officer.officer_doctor_code`, when present.
    pub doctor_code: Option<String>,
    /// App role. HOSxP group permissions are empty, so every signed-in
    /// officer currently gets the same role.
    pub role: String,
}

/// Returns the lowercase 32-character MD5 hex digest of `password`.
pub fn md5_hex(password: &str) -> String {
    format!("{:x}", Md5::digest(password.as_bytes()))
}

/// True when `password` hashes to the stored 32-hex MD5 digest.
///
/// The stored value is validated for shape and compared case-insensitively.
/// A missing, short, or non-hex digest never matches.
pub fn verify_password(password: &str, stored_md5: &str) -> bool {
    let stored = stored_md5.trim();
    if stored.len() != 32 || !stored.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    md5_hex(password).eq_ignore_ascii_case(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A non-secret test vector: verifies the algorithm, not a credential.
    const TEST_INPUT: &str = "med-recon-test";
    const TEST_INPUT_MD5: &str = "2b0294858ab9daeabf6c01979594c4d5";

    #[test]
    fn md5_hex_matches_a_known_digest() {
        assert_eq!(md5_hex(TEST_INPUT), TEST_INPUT_MD5);
    }

    #[test]
    fn verify_accepts_lower_and_upper_hex() {
        assert!(verify_password(TEST_INPUT, TEST_INPUT_MD5));
        assert!(verify_password(
            TEST_INPUT,
            &md5_hex(TEST_INPUT).to_uppercase()
        ));
    }

    #[test]
    fn verify_rejects_wrong_or_malformed_hashes() {
        assert!(!verify_password("different-input", TEST_INPUT_MD5));
        assert!(!verify_password(TEST_INPUT, ""));
        assert!(!verify_password(TEST_INPUT, "not-a-md5"));
        let malformed = format!("{}z", &TEST_INPUT_MD5[..31]);
        assert!(!verify_password(TEST_INPUT, &malformed));
    }

    #[test]
    fn officer_debug_redacts_the_password_hash() {
        let digest = md5_hex(TEST_INPUT);
        let account = OfficerAccount {
            officer_id: 1,
            login_name: "pharm".into(),
            password_md5: digest.clone(),
            active: "Y".into(),
            auth_type_id: 1,
            doctor_code: None,
            display_name: "Pharmacist".into(),
        };
        let debug = format!("{account:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&digest));
    }
}
