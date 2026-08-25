//! Top bar — 44px header: brand, live connection status, settings button.

use leptos::prelude::*;

use crate::components::icons::{IconLogo, IconSettings};
use crate::state::{AppState, ConnectionHealth};

#[component]
pub fn TopBar(state: AppState) -> impl IntoView {
    let on_settings = move |_| state.settings_open.set(true);

    let site_name = move || {
        let name = state.site_name.get();
        if name.is_empty() {
            "Med Recon".to_string()
        } else {
            format!("Med Recon {name}")
        }
    };

    let status_text = move || match state.health.get() {
        ConnectionHealth::Connected => "เชื่อมต่อแล้ว".to_string(),
        ConnectionHealth::Disconnected => "ไม่สามารถเชื่อมต่อได้".to_string(),
        ConnectionHealth::Unconfigured => "ยังไม่ได้ตั้งค่า".to_string(),
    };

    view! {
        <header class="top-bar">
            <div class="top-bar__left">
                <IconLogo class="top-bar__logo" />
                <h1 class="top-bar__title">{site_name}</h1>
            </div>
            <div class="top-bar__right">
                <span class="top-bar__status">
                    <span
                        class:top-bar__status-dot=true
                        class:top-bar__status-dot--disconnected=move || {
                            state.health.get() == ConnectionHealth::Disconnected
                        }
                    ></span>
                    <span class="top-bar__status-text">{status_text}</span>
                </span>
                <button class="top-bar__button" on:click=on_settings>
                    <IconSettings class="icon" />
                    "ตั้งค่า"
                </button>
            </div>
        </header>
    }
}
