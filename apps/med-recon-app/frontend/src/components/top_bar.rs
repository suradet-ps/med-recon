//! Top bar - 44px header: brand, live connection status, signed-in officer,
//! settings, and logout.

use leptos::prelude::*;

use crate::api;
use crate::components::icons::{IconHelp, IconLogOut, IconLogo, IconSettings};
use crate::state::{AppState, ConnectionHealth};

#[component]
pub fn TopBar(state: AppState) -> impl IntoView {
    let on_settings = move |_| state.settings_open.set(true);
    let on_help = move |_| state.help_open.set(true);
    let on_logout = move |_| {
        leptos::task::spawn_local(async move {
            let _ = api::logout().await;
            state.clear_session();
        });
    };

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
                {move || {
                    state
                        .session
                        .get()
                        .map(|user| {
                            let initial = user
                                .display_name
                                .chars()
                                .next()
                                .or_else(|| user.login_name.chars().next())
                                .map(|c| c.to_string())
                                .unwrap_or_default();
                            let title = format!("{} · {}", user.display_name, user.login_name);
                            view! {
                                <span class="user-chip" title=title>
                                    <span class="user-chip__avatar">{initial}</span>
                                    <span class="user-chip__name">{user.display_name}</span>
                                </span>
                            }
                        })
                }}
                <button class="top-bar__button" on:click=on_help>
                    <IconHelp class="icon" />
                    "คู่มือ"
                </button>
                <button class="top-bar__button" on:click=on_settings>
                    <IconSettings class="icon" />
                    "ตั้งค่า"
                </button>
                <button
                    class="top-bar__button top-bar__logout"
                    title="ออกจากระบบ"
                    on:click=on_logout
                >
                    <IconLogOut class="icon" />
                    "ออกจากระบบ"
                </button>
            </div>
        </header>
    }
}
