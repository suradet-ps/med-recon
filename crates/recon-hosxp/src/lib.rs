//! Read-only HOSxP (MySQL/MariaDB) repository.
//!
//! Implements the persistence boundary for Recon against a HOSxP database:
//! patient identity search, OPD/IPD dispensing history, allergy records, and
//! visit history. **Every statement is validated as read-only before
//! execution** — the code refuses to run anything other than `SELECT`,
//! `SHOW`, `DESCRIBE`, or `EXPLAIN`, regardless of the configured DB user.
//!
//! Schema note: table/column names follow the HOSxP schema reference in
//! AGENTS.md. Queries tolerate per-site variations: missing tables
//! (MySQL 1146) or columns (1054) skip the affected section with a
//! user-visible warning instead of failing the load.

pub mod client;
pub mod config;
pub mod error;
pub mod queries;
pub mod readonly;

pub use client::HosxpClient;
pub use config::HosxpConfig;
pub use error::Error;
