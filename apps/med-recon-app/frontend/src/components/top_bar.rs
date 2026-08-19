//! Top bar — 44px header: brand, live connection status, language toggle,
//! settings button.

use leptos::prelude::*;

use crate::components::icons::{IconLogo, IconSettings};
use crate::i18n::tr;
use crate::state::{AppState, ConnectionHealth};

#[component]
pub fn TopBar(state: AppState) -> impl IntoView {
    let lang_label = move || state.lang.get().label();
    let on_lang = move |_| {
        let next = state.lang.get().toggle();
        next.save();
        state.lang.set(next);
    };
    let on_settings = move |_| state.settings_open.set(true);

    let status_text = move || match state.health.get() {
        ConnectionHealth::Connected => tr(state.lang.get(), "top.health.connected").to_string(),
        ConnectionHealth::Disconnected => {
            tr(state.lang.get(), "top.health.disconnected").to_string()
        }
        ConnectionHealth::Unconfigured => {
            tr(state.lang.get(), "top.health.unconfigured").to_string()
        }
    };

    view! {
        <header class="top-bar">
            <div class="top-bar__left">
                <IconLogo class="top-bar__logo" />
                <h1 class="top-bar__title">{move || tr(state.lang.get(), "app.name")}</h1>
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
                <button class="top-bar__button" on:click=on_lang>{lang_label}</button>
                <button class="top-bar__button" on:click=on_settings>
                    <IconSettings class="icon" />
                    {move || tr(state.lang.get(), "top.settings")}
                </button>
            </div>
        </header>
    }
}
