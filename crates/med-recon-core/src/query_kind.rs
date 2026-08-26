//! Detection of what kind of identifier a patient-search input is.

/// The three input shapes the single patient search box accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QueryKind {
    /// 13-digit Thai national ID.
    Cid,
    /// Hospital HN.
    Hn,
    /// Anything else - treated as a name search.
    Name,
}

/// Classifies a patient-search input.
///
/// Rules: exactly 13 digits → CID; 5–10 digits only → HN; anything else →
/// name. The HN length rule is a documented default - confirm the exact HN
/// pattern of the target hospital before relying on it.
pub fn detect_query_kind(input: &str) -> QueryKind {
    let trimmed = input.trim();
    if trimmed.len() == 13 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        QueryKind::Cid
    } else if !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_digit())
        && (5..=10).contains(&trimmed.len())
    {
        QueryKind::Hn
    } else {
        QueryKind::Name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirteen_digits_is_cid() {
        assert_eq!(detect_query_kind("1101701234567"), QueryKind::Cid);
    }

    #[test]
    fn cid_ignores_surrounding_whitespace() {
        assert_eq!(detect_query_kind(" 1101701234567 "), QueryKind::Cid);
    }

    #[test]
    fn short_digit_string_is_hn() {
        assert_eq!(detect_query_kind("12345678"), QueryKind::Hn);
        assert_eq!(detect_query_kind("12345"), QueryKind::Hn);
    }

    #[test]
    fn too_short_or_long_digits_fall_back_to_name() {
        assert_eq!(detect_query_kind("123"), QueryKind::Name);
        assert_eq!(detect_query_kind("12345678901234567890"), QueryKind::Name);
    }

    #[test]
    fn mixed_alphanumeric_is_name() {
        assert_eq!(detect_query_kind("สมชาย ใจดี"), QueryKind::Name);
        assert_eq!(detect_query_kind("HN12345"), QueryKind::Name);
    }

    #[test]
    fn empty_or_whitespace_is_name() {
        assert_eq!(detect_query_kind(""), QueryKind::Name);
        assert_eq!(detect_query_kind("   "), QueryKind::Name);
    }
}
