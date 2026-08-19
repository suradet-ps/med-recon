//! PHI redaction helpers.
//!
//! HN, CID, and patient names must never appear in logs or crash reports.
//! Use these helpers before placing any patient-identifying value into a
//! log or error context.

/// Redact a hospital number for logging: keep only the last 4 digits.
///
/// # Examples
///
/// ```rust
/// assert_eq!(med_recon_core::redact_hn("0012345"), "****2345");
/// ```
pub fn redact_hn(hn: &str) -> String {
    let hn = hn.trim();
    if hn.len() <= 4 {
        "****".to_string()
    } else {
        format!("****{}", &hn[hn.len() - 4..])
    }
}

/// Redact a CID (national ID, 13 digits) for logging: keep only the last 4.
pub fn redact_cid(cid: &str) -> String {
    let cid = cid.trim();
    if cid.len() <= 4 {
        "*********".to_string()
    } else {
        format!("*********{}", &cid[cid.len() - 4..])
    }
}

/// Redact a patient display name entirely — names are never logged.
pub const REDACTED_NAME: &str = "***";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_hn_keeping_last_four() {
        assert_eq!(redact_hn("0012345"), "****2345");
        assert_eq!(redact_hn(" 0012345 "), "****2345");
    }

    #[test]
    fn redacts_short_hn_fully() {
        assert_eq!(redact_hn("123"), "****");
    }

    #[test]
    fn redacts_cid_keeping_last_four() {
        assert_eq!(redact_cid("1103700123456"), "*********3456");
    }

    #[test]
    fn redacts_short_cid() {
        assert_eq!(redact_cid("12"), "*********");
    }
}
