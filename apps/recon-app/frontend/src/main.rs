//! Recon frontend entry point (Leptos 0.8 CSR).
//!
//! This crate is compiled for `wasm32-unknown-unknown` (via `trunk`) and
//! embedded in the Tauri shell. The native target is only used for
//! workspace-wide lint/test commands, where the components are unreachable
//! — hence the dead-code allowance below.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod api;
mod app;
mod components;
mod i18n;
mod state;

#[cfg(target_arch = "wasm32")]
fn main() {
    app::run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("recon-frontend is a WASM-only crate; build it with `trunk build`");
}
