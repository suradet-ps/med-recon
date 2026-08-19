//! Read-only enforcement for every statement executed against HOSxP.
//!
//! This is a defense-in-depth guard on top of the read-only DB role: no
//! `INSERT`/`UPDATE`/`DELETE`/`DROP`/DDL ever reaches the server, no matter
//! what statement string is passed to the client.

use crate::error::{Error, Result};

/// SQL keywords that are allowed to reach a HOSxP read-only connection.
const ALLOWED_KEYWORDS: [&str; 4] = ["select", "show", "describe", "explain"];

/// Validate that a statement is read-only, returning the leading keyword.
///
/// The check is intentionally strict: the statement must *begin* with one of
/// the allowed keywords. Anything else (including `WITH`, which can wrap DML)
/// is rejected.
pub fn assert_read_only(stmt: &str) -> Result<()> {
    let first = stmt
        .split_whitespace()
        .next()
        .map(str::to_lowercase)
        .unwrap_or_default();
    if ALLOWED_KEYWORDS.contains(&first.as_str()) {
        Ok(())
    } else {
        Err(Error::ReadOnlyViolation(
            stmt.trim().chars().take(64).collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select_statements() {
        assert!(assert_read_only("SELECT hn FROM patient").is_ok());
        assert!(assert_read_only("  select hn from patient").is_ok());
    }

    #[test]
    fn allows_diagnostic_keywords() {
        assert!(assert_read_only("SHOW TABLES").is_ok());
        assert!(assert_read_only("DESCRIBE patient").is_ok());
        assert!(assert_read_only("EXPLAIN SELECT 1").is_ok());
    }

    #[test]
    fn rejects_all_dml_and_ddl() {
        for stmt in [
            "INSERT INTO patient (hn) VALUES ('x')",
            "UPDATE patient SET fname = 'x'",
            "DELETE FROM patient",
            "DROP TABLE patient",
            "ALTER TABLE patient ADD COLUMN x INT",
            "CREATE TABLE evil (x INT)",
            "WITH cte AS (SELECT 1) DELETE FROM patient",
            "TRUNCATE TABLE patient",
        ] {
            assert!(assert_read_only(stmt).is_err(), "must reject: {stmt}");
        }
    }

    #[test]
    fn rejects_empty_statement() {
        assert!(assert_read_only("").is_err());
        assert!(assert_read_only("   ").is_err());
    }
}
