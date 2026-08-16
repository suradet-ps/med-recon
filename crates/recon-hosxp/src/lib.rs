//! Read-only HOSxP (MySQL/MariaDB) repository.
//!
//! Implements the persistence boundary for Recon against a HOSxP database:
//! patient identity search, OPD/IPD dispensing history, allergy records, and
//! visit history. **Every statement is validated as read-only before
//! execution** — the code refuses to run anything other than `SELECT`,
//! `SHOW`, `DESCRIBE`, or `EXPLAIN`, regardless of the configured DB user.
//!
//! Schema note: table/column names follow the HOSxP schema reference in
//! AGENTS.md. Columns not listed there (e.g. `medusage` sig columns) are
//! gated behind a site config flag and must be confirmed against the live
//! schema before enabling.

pub mod client;
pub mod config;
pub mod error;
pub mod queries;
pub mod readonly;

pub use client::HosxpClient;
pub use config::HosxpConfig;
pub use error::Error;
