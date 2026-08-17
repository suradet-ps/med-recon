//! Settings dialog — two independent sections:
//!
//! 1. **HOSxP connection** — host/port/database/user/password. Stored
//!    encrypted (`connection.json`, AES-256-GCM + OS keychain master key).
//! 2. **Site settings** — site name, history window, and the current
//!    medication list. Stored as plain JSON (`settings.json`).
//!
//! Test runs against the typed connection values; verification happens
//! before anything is saved.

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ConnectionInput, DrugInfo, SiteSettings};
use crate::components::icons::{IconCheckCircle, IconPlug, IconSave, IconSearch, IconX};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;

/// Default MySQL port, prefilled in the port field.
const DEFAULT_PORT: u16 = 3306;

/// Debounce window for the drug picker search (milliseconds).
const DRUG_SEARCH_DEBOUNCE_MS: u64 = 250;

/// Default history window in days, prefilled in the settings section.
const DEFAULT_HISTORY_DAYS: u32 = 730;

/// The two settings tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Connection,
    Site,
}

#[component]
pub fn SettingsModal(state: AppState) -> impl IntoView {
    let lang = state.lang;

    // --- Connection section -------------------------------------------------
    let host = RwSignal::new(String::new());
    let port = RwSignal::new(DEFAULT_PORT.to_string());
    let database = RwSignal::new("hos".to_string());
    let user = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let conn_message = RwSignal::new(None::<(bool, String)>);
    let conn_busy = RwSignal::new(false);

    // --- Site settings section ----------------------------------------------
    let site_name = RwSignal::new(String::new());
    let history_days = RwSignal::new(DEFAULT_HISTORY_DAYS);
    let selected_meds = RwSignal::new(Vec::<DrugInfo>::new());
    let settings_message = RwSignal::new(None::<(bool, String)>);
    let settings_busy = RwSignal::new(false);
    let tab = RwSignal::new(SettingsTab::Connection);

    // Load the saved site settings + current medication list on mount.
    spawn_local(async move {
        match api::get_site_settings().await {
            Ok(settings) => {
                site_name.set(settings.site_name);
                history_days.set(settings.history_days);
            }
            Err(e) => settings_message.set(Some((false, e.message))),
        }
    });

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

    let wipe_connection_fields = move || {
        wipe_field(&host);
        wipe_field(&port);
        wipe_field(&database);
        wipe_field(&user);
        wipe_field(&password);
    };

    let close = move || {
        wipe_connection_fields();
        conn_message.set(None);
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

    let build_connection_input = move || -> Result<ConnectionInput, String> {
        let port_value = port
            .get_untracked()
            .trim()
            .parse::<u16>()
            .map_err(|_| tr(lang.get_untracked(), "settings.error_port").to_string())?;
        let input = ConnectionInput {
            host: host.get_untracked(),
            port: port_value,
            database: database.get_untracked(),
            user: user.get_untracked(),
            password: password.get_untracked(),
        };
        if input.host.trim().is_empty()
            || input.database.trim().is_empty()
            || input.user.trim().is_empty()
        {
            return Err(tr(lang.get_untracked(), "settings.error_required").to_string());
        }
        Ok(input)
    };

    let run_test = move || {
        if conn_busy.get_untracked() {
            return;
        }
        let input = match build_connection_input() {
            Ok(input) => input,
            Err(err_message) => {
                conn_message.set(Some((false, err_message)));
                return;
            }
        };
        conn_busy.set(true);
        conn_message.set(None);
        spawn_local(async move {
            match api::test_connection(Some(&input)).await {
                Ok(result) => conn_message.set(Some((
                    true,
                    tr_f(
                        lang.get_untracked(),
                        "settings.test_ok",
                        &[("ms", &result.latency_ms.to_string())],
                    ),
                ))),
                Err(error) => conn_message.set(Some((false, error.message))),
            }
            conn_busy.set(false);
        });
    };

    let run_save_connection = move || {
        if conn_busy.get_untracked() {
            return;
        }
        let input = match build_connection_input() {
            Ok(input) => input,
            Err(err_message) => {
                conn_message.set(Some((false, err_message)));
                return;
            }
        };
        conn_busy.set(true);
        conn_message.set(None);
        spawn_local(async move {
            match api::save_connection(&input).await {
                Ok(()) => {
                    wipe_connection_fields();
                    conn_message.set(Some((
                        true,
                        tr(lang.get_untracked(), "settings.save_ok").to_string(),
                    )));
                    state.configured.set(true);
                    state.health.set(ConnectionHealth::Connected);
                }
                Err(error) => conn_message.set(Some((false, error.message))),
            }
            conn_busy.set(false);
        });
    };

    let run_save_settings = move || {
        if settings_busy.get_untracked() {
            return;
        }
        settings_busy.set(true);
        settings_message.set(None);
        let settings = SiteSettings {
            site_name: site_name.get_untracked(),
            history_days: history_days.get_untracked(),
            current_med_codes: selected_meds
                .get_untracked()
                .iter()
                .map(|d| d.icode.clone())
                .collect(),
        };
        spawn_local(async move {
            match api::save_site_settings(&settings).await {
                Ok(()) => settings_message.set(Some((
                    true,
                    tr(lang.get_untracked(), "settings.save_ok").to_string(),
                ))),
                Err(e) => settings_message.set(Some((false, e.message))),
            }
            settings_busy.set(false);
        });
    };

    let connection_panel = move || {
        view! {
            <section class="form-section">
                <h3 class="form-section__title">
                    <IconPlug class="icon" />
                    {move || tr(lang.get(), "settings.section_connection")}
                </h3>

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

                {move || {
                    conn_message.get().map(|(is_success, text)| {
                        let class = if is_success {
                            "modal__message modal__message--success"
                        } else {
                            "modal__message modal__message--error"
                        };
                        view! { <p class=class>{text}</p> }
                    })
                }}

                <div class="modal__actions" style="margin-top:var(--sp-md)">
                    <button
                        class="button-secondary button-secondary--inline"
                        on:click=move |_| run_test()
                        prop:disabled=move || conn_busy.get()
                    >
                        <IconPlug class="icon" />
                        {move || if conn_busy.get() { tr(lang.get(), "settings.testing") } else { tr(lang.get(), "settings.test") }}
                    </button>
                    <button
                        class="button-primary button-primary--inline"
                        on:click=move |_| run_save_connection()
                        prop:disabled=move || conn_busy.get()
                    >
                        <IconSave class="icon" />
                        {move || if conn_busy.get() { tr(lang.get(), "settings.saving") } else { tr(lang.get(), "settings.save") }}
                    </button>
                </div>

                <p class="modal__note">{move || tr(lang.get(), "settings.note")}</p>
            </section>
        }
        .into_any()
    };

    let site_panel = move || {
        view! {
            <section class="form-section">
                <h3 class="form-section__title">
                    <IconCheckCircle class="icon" />
                    {move || tr(lang.get(), "settings.section_site")}
                </h3>

                <div class="form-field">
                    <label for="cfg-site">{move || tr(lang.get(), "settings.site_name")}</label>
                    <input
                        id="cfg-site"
                        class="form-input"
                        placeholder={move || tr(lang.get(), "settings.site_name_placeholder")}
                        prop:value=move || site_name.get()
                        on:input=move |ev| site_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
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

                <p class="modal__note">{move || tr(lang.get(), "settings.meds_note")}</p>
                <CurrentMedsPanel state=state selected=selected_meds/>

                {move || {
                    settings_message.get().map(|(is_success, text)| {
                        let class = if is_success {
                            "modal__message modal__message--success"
                        } else {
                            "modal__message modal__message--error"
                        };
                        view! { <p class=class>{text}</p> }
                    })
                }}

                <div class="modal__actions" style="margin-top:var(--sp-md)">
                    <button
                        class="button-secondary button-secondary--inline"
                        on:click=move |_| close()
                    >
                        <IconX class="icon" />
                        {move || tr(lang.get(), "settings.cancel")}
                    </button>
                    <button
                        class="button-primary button-primary--inline"
                        on:click=move |_| run_save_settings()
                        prop:disabled=move || settings_busy.get()
                    >
                        <IconSave class="icon" />
                        {move || if settings_busy.get() { tr(lang.get(), "settings.saving") } else { tr(lang.get(), "settings.save_settings") }}
                    </button>
                </div>
            </section>
        }
        .into_any()
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

                <div class="tabs">
                    <button
                        class=move || {
                            if tab.get() == SettingsTab::Connection {
                                "tab tab--active"
                            } else {
                                "tab"
                            }
                        }
                        on:click=move |_| tab.set(SettingsTab::Connection)
                    >
                        <IconPlug class="icon" />
                        {move || tr(lang.get(), "settings.tab_connection")}
                    </button>
                    <button
                        class=move || {
                            if tab.get() == SettingsTab::Site {
                                "tab tab--active"
                            } else {
                                "tab"
                            }
                        }
                        on:click=move |_| tab.set(SettingsTab::Site)
                    >
                        <IconCheckCircle class="icon" />
                        {move || tr(lang.get(), "settings.tab_site")}
                    </button>
                </div>

                {move || {
                    if tab.get() == SettingsTab::Connection {
                        connection_panel()
                    } else {
                        site_panel()
                    }
                }}
            </section>
        </div>
    }
}

// Re-exported so `lib.rs` can set health without an extra import.
pub use crate::state::ConnectionHealth;

/// The operator-configured current-medication picker.
///
/// Searches `drugitems` via the backend and edits a locally held selection
/// (the shared `selected` signal). Persistence happens through the parent
/// section's save button — this panel only loads and edits.
#[component]
fn CurrentMedsPanel(state: AppState, selected: RwSignal<Vec<DrugInfo>>) -> impl IntoView {
    let lang = state.lang;
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<DrugInfo>::new());
    let searching = RwSignal::new(false);
    let searched = RwSignal::new(false);
    let load_error = RwSignal::new(None::<String>);

    spawn_local(async move {
        match api::get_current_meds().await {
            Ok(list) => selected.set(list),
            Err(e) => load_error.set(Some(e.message)),
        }
    });

    let add = move |drug: DrugInfo| {
        let mut current = selected.get_untracked();
        if !current.iter().any(|d| d.icode == drug.icode) {
            current.push(drug);
            selected.set(current);
        }
    };

    let remove = move |icode: String| {
        selected.set(
            selected
                .get_untracked()
                .into_iter()
                .filter(|d| d.icode != icode)
                .collect(),
        );
    };

    let run_search = move || {
        let q = query.get_untracked().trim().to_string();
        if q.len() < 2 {
            results.set(Vec::new());
            searched.set(false);
            return;
        }
        searching.set(true);
        spawn_local(async move {
            match api::search_drugs(&q).await {
                Ok(list) => results.set(list),
                Err(e) => {
                    results.set(Vec::new());
                    load_error.set(Some(e.message));
                }
            }
            searched.set(true);
            searching.set(false);
        });
    };

    let last_timeout = RwSignal::new(None::<leptos::prelude::TimeoutHandle>);
    let on_input = move |value: String| {
        query.set(value);
        if let Some(prev) = last_timeout.get() {
            prev.clear();
        }
        last_timeout.set(Some(
            set_timeout_with_handle(
                run_search,
                std::time::Duration::from_millis(DRUG_SEARCH_DEBOUNCE_MS),
            )
            .expect("invariant: setTimeout is available in the Tauri webview"),
        ));
    };

    view! {
        <div>
            {move || {
                load_error.get().map(|msg| view! {
                    <p class="modal__note">{msg}</p>
                })
            }}

            <div class="search-wrapper" style="margin:6px 0">
                <IconSearch class="search-icon" />
                <input
                    class="search-input"
                    placeholder={move || tr(lang.get(), "settings.meds_search")}
                    prop:value=move || query.get()
                    on:input=move |ev| on_input(event_target_value(&ev))
                />
            </div>

            {move || {
                if searching.get() {
                    view! { <p class="modal__note">"…"</p> }.into_any()
                } else if searched.get() && results.get().is_empty() {
                    view! { <p class="modal__note">{tr(lang.get(), "settings.meds_no_results")}</p> }.into_any()
                } else if !results.get().is_empty() {
                    let count = results.get().len();
                    view! {
                        <>
                            <p class="modal__note">
                                {tr_f(lang.get(), "settings.meds_results", &[("n", &count.to_string())])}
                            </p>
                            <ul class="result-list">
                                {results.get().iter().map(|d| {
                                    let drug = d.clone();
                                    let label = drug_label(&drug);
                                    view! {
                                        <li class="search-result-row">
                                            <span class="search-result-row__name">{label}</span>
                                            <span class="search-result-row__code">{drug.icode.clone()}</span>
                                            <button
                                                class="button-secondary button-secondary--inline"
                                                on:click=move |_| add(drug.clone())
                                            >
                                                {move || tr(lang.get(), "settings.meds_add")}
                                            </button>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        </>
                    }.into_any()
                } else {
                    view! { <span hidden></span> }.into_any()
                }
            }}

            <p class="sidebar__label">
                {move || tr_f(lang.get(), "settings.meds_selected", &[("n", &selected.get().len().to_string())])}
            </p>
            <ul class="result-list">
                {move || {
                    let list = selected.get();
                    if list.is_empty() {
                        view! { <p class="modal__note">{tr(lang.get(), "settings.meds_empty")}</p> }.into_any()
                    } else {
                        list.iter().map(|d| {
                            let drug = d.clone();
                            let label = drug_label(&drug);
                            view! {
                                <li class="search-result-row">
                                    <span class="search-result-row__name">{label}</span>
                                    <span class="search-result-row__code">{drug.icode.clone()}</span>
                                    <button
                                        class="button-secondary button-secondary--inline"
                                        on:click=move |_| remove(drug.icode.clone())
                                    >
                                        <IconX class="icon" />
                                        {move || tr(lang.get(), "settings.meds_remove")}
                                    </button>
                                </li>
                            }
                        }).collect_view().into_any()
                    }
                }}
            </ul>
        </div>
    }
}

/// "Name · strength units" label for a drug entry.
fn drug_label(d: &DrugInfo) -> String {
    let mut label = d.name.clone();
    if let Some(strength) = &d.strength {
        label.push_str(&format!(" · {strength}"));
    }
    if let Some(units) = &d.units {
        label.push(' ');
        label.push_str(units);
    }
    label
}
