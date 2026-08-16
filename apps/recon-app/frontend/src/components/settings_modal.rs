//! Connection settings dialog — adapted from AllerX.
//!
//! Opened from the top bar (or automatically on first launch when no
//! settings exist). Values are sent to the backend, which encrypts them
//! before anything touches disk. Test runs against the typed values —
//! verification happens before anything is saved.

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ConnectionInput};
use crate::components::icons::{IconPlug, IconSave, IconX};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use recon_core::DateEra;

/// Default MySQL port, prefilled in the port field.
const DEFAULT_PORT: u16 = 3306;

#[component]
pub fn SettingsModal(state: AppState) -> impl IntoView {
    let site_name = RwSignal::new(String::new());
    let host = RwSignal::new(String::new());
    let port = RwSignal::new(DEFAULT_PORT.to_string());
    let database = RwSignal::new("hos".to_string());
    let user = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let era = RwSignal::new(DateEra::Christian);
    let history_days = RwSignal::new(730u32);
    let use_medusage_sig = RwSignal::new(false);
    let message = RwSignal::new(None::<(bool, String)>);
    let busy = RwSignal::new(false);

    let lang = state.lang;

    /// Zeroizes an operator-typed field before dropping it — plain
    /// `String::new()` only frees the buffer, leaving the plaintext bytes
    /// behind for a memory scan.
    fn wipe_field(signal: &RwSignal<String>) {
        let mut value = signal.get_untracked();
        if !value.is_empty() {
            unsafe { value.as_mut_vec().fill(0) };
        }
        signal.set(String::new());
    }

    let wipe_fields = move || {
        wipe_field(&site_name);
        wipe_field(&host);
        wipe_field(&port);
        wipe_field(&database);
        wipe_field(&user);
        wipe_field(&password);
    };

    let close = move || {
        wipe_fields();
        message.set(None);
        state.settings_open.set(false);
    };

    // Escape closes the dialog from anywhere.
    let open_flag = state.settings_open;
    let close_on_escape = move |event: ev::KeyboardEvent| {
        if event.key() == "Escape" && open_flag.get_untracked() {
            close();
        }
    };
    let escape_handle = window_event_listener(ev::keydown, close_on_escape);
    let _escape_handle = StoredValue::new(escape_handle);

    let build_input = move || -> Result<ConnectionInput, String> {
        let port_value = port
            .get_untracked()
            .trim()
            .parse::<u16>()
            .map_err(|_| tr(lang.get_untracked(), "settings.error_port").to_string())?;
        let input = ConnectionInput {
            site_name: site_name.get_untracked(),
            host: host.get_untracked(),
            port: port_value,
            database: database.get_untracked(),
            user: user.get_untracked(),
            password: password.get_untracked(),
            era: era.get_untracked(),
            history_days: history_days.get_untracked(),
            use_medusage_sig: use_medusage_sig.get_untracked(),
        };
        if input.site_name.trim().is_empty()
            || input.host.trim().is_empty()
            || input.database.trim().is_empty()
            || input.user.trim().is_empty()
        {
            return Err(tr(lang.get_untracked(), "settings.error_required").to_string());
        }
        Ok(input)
    };

    let run_test = move || {
        if busy.get_untracked() {
            return;
        }
        let input = match build_input() {
            Ok(input) => input,
            Err(err_message) => {
                message.set(Some((false, err_message)));
                return;
            }
        };
        busy.set(true);
        message.set(None);
        spawn_local(async move {
            match api::test_connection(Some(&input)).await {
                Ok(result) => message.set(Some((
                    true,
                    tr_f(
                        lang.get_untracked(),
                        "settings.test_ok",
                        &[("ms", &result.latency_ms.to_string())],
                    ),
                ))),
                Err(error) => message.set(Some((false, error.message))),
            }
            busy.set(false);
        });
    };

    let run_save = move || {
        let input = match build_input() {
            Ok(input) => input,
            Err(err_message) => {
                message.set(Some((false, err_message)));
                return;
            }
        };
        busy.set(true);
        message.set(None);
        spawn_local(async move {
            match api::save_site_config(&input).await {
                Ok(()) => {
                    wipe_fields();
                    message.set(Some((
                        true,
                        tr(lang.get_untracked(), "settings.save_ok").to_string(),
                    )));
                    state.configured.set(true);
                    state.health.set(ConnectionHealth::Connected);
                    state.settings_open.set(false);
                }
                Err(error) => message.set(Some((false, error.message))),
            }
            busy.set(false);
        });
    };

    view! {
        <div
            class="modal-backdrop"
            style:display=move || {
                if state.settings_open.get() {
                    "flex"
                } else {
                    "none"
                }
            }
            on:click=move |_| close()
        >
            <section class="modal" on:click=move |ev| ev.stop_propagation()>
                <h2 class="modal__title">{move || tr(lang.get(), "settings.title")}</h2>
                <p class="modal__status">
                    {move || {
                        if state.configured.get() {
                            tr(lang.get(), "settings.status_ok")
                        } else {
                            tr(lang.get(), "settings.status_none")
                        }
                    }}
                </p>

                <div class="form-field">
                    <label for="cfg-site">{move || tr(lang.get(), "settings.site_name")}</label>
                    <input
                        id="cfg-site"
                        class="form-input"
                        prop:value=move || site_name.get()
                        on:input=move |ev| site_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
                    <label for="cfg-host">{move || tr(lang.get(), "settings.host")}</label>
                    <input
                        id="cfg-host"
                        class="form-input form-input--mono"
                        placeholder="192.168.1.10"
                        prop:value=move || host.get()
                        on:input=move |ev| host.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-row">
                    <div class="form-field" style="max-width:100px">
                        <label for="cfg-port">{move || tr(lang.get(), "settings.port")}</label>
                        <input
                            id="cfg-port"
                            class="form-input form-input--mono"
                            prop:value=move || port.get()
                            on:input=move |ev| port.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-field form-field--grow">
                        <label for="cfg-database">{move || tr(lang.get(), "settings.database")}</label>
                        <input
                            id="cfg-database"
                            class="form-input form-input--mono"
                            placeholder="hos"
                            prop:value=move || database.get()
                            on:input=move |ev| database.set(event_target_value(&ev))
                        />
                    </div>
                </div>
                <div class="form-field">
                    <label for="cfg-user">{move || tr(lang.get(), "settings.user")}</label>
                    <input
                        id="cfg-user"
                        class="form-input form-input--mono"
                        placeholder="recon_ro"
                        prop:value=move || user.get()
                        on:input=move |ev| user.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
                    <label for="cfg-password">{move || tr(lang.get(), "settings.password")}</label>
                    <input
                        id="cfg-password"
                        class="form-input form-input--mono"
                        type="password"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                    />
                </div>

                <div class="form-row">
                    <div class="form-field">
                        <label for="cfg-era">{move || tr(lang.get(), "settings.era")}</label>
                        <select
                            id="cfg-era"
                            class="form-input"
                            prop:value=move || {
                                if era.get() == DateEra::Buddhist {
                                    "buddhist".to_string()
                                } else {
                                    "christian".to_string()
                                }
                            }
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                era.set(if v == "buddhist" {
                                    DateEra::Buddhist
                                } else {
                                    DateEra::Christian
                                });
                            }
                        >
                            <option value="christian">{move || tr(lang.get(), "settings.era_ce")}</option>
                            <option value="buddhist">{move || tr(lang.get(), "settings.era_be")}</option>
                        </select>
                    </div>
                    <div class="form-field form-field--grow">
                        <label for="cfg-window">{move || tr(lang.get(), "settings.history_days")}</label>
                        <input
                            id="cfg-window"
                            class="form-input form-input--mono"
                            type="number"
                            min="30"
                            max="3650"
                            prop:value=move || history_days.get().to_string()
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                    history_days.set(v);
                                }
                            }
                        />
                    </div>
                </div>

                <div class="form-field">
                    <label class="check-row">
                        <input
                            type="checkbox"
                            prop:checked=move || use_medusage_sig.get()
                            on:change=move |ev| use_medusage_sig.set(event_target_checked(&ev))
                        />
                        <span>{move || tr(lang.get(), "settings.medusage")}</span>
                    </label>
                    <p class="modal__note">{move || tr(lang.get(), "settings.medusage_note")}</p>
                </div>

                {move || {
                    message.get().map(|(is_success, text)| {
                        let class = if is_success {
                            "modal__message modal__message--success"
                        } else {
                            "modal__message modal__message--error"
                        };
                        view! { <p class=class>{text}</p> }
                    })
                }}

                <div class="modal__actions">
                    <button
                        class="button-secondary button-secondary--inline"
                        on:click=move |_| run_test()
                        prop:disabled=move || busy.get()
                    >
                        <IconPlug class="icon" />
                        {move || if busy.get() { tr(lang.get(), "settings.testing") } else { tr(lang.get(), "settings.test") }}
                    </button>
                    <button
                        class="button-secondary button-secondary--inline"
                        on:click=move |_| close()
                        prop:disabled=move || busy.get()
                    >
                        <IconX class="icon" />
                        {move || tr(lang.get(), "settings.cancel")}
                    </button>
                    <button
                        class="button-primary button-primary--inline"
                        on:click=move |_| run_save()
                        prop:disabled=move || busy.get()
                    >
                        <IconSave class="icon" />
                        {move || if busy.get() { tr(lang.get(), "settings.saving") } else { tr(lang.get(), "settings.save") }}
                    </button>
                </div>

                <p class="modal__note">{move || tr(lang.get(), "settings.note")}</p>
            </section>
        </div>
    }
}

// Re-exported so `lib.rs` can set health without an extra import.
pub use crate::state::ConnectionHealth;
