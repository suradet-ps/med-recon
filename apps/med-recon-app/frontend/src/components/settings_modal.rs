//! Settings dialog - two independent sections:
//!
//! 1. **HOSxP connection** - host/port/database/user/password. Stored
//!    encrypted (`connection.json`, AES-256-GCM + OS keychain master key).
//! 2. **Site settings** - site name, history window, and the current
//!    medication list. Stored as plain JSON (`settings.json`).
//!
//! Test runs against the typed connection values; verification happens
//! before anything is saved.

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{self, ConnectionInput, DrugInfo, SiteSettings};
use crate::components::icons::{IconCheckCircle, IconPlug, IconSave, IconSearch, IconX};
use crate::state::AppState;

/// Default MySQL port, prefilled in the port field.
const DEFAULT_PORT: u16 = 3306;

/// Debounce window for the drug picker search (milliseconds).
const DRUG_SEARCH_DEBOUNCE_MS: u64 = 250;

/// Default history window in days, prefilled in the settings section.
const DEFAULT_HISTORY_DAYS: u32 = 730;

/// Hard ceiling for any backend operation (connect, keychain, save) so the
/// busy state can never stick forever if the backend future is dropped.
const OPERATION_TIMEOUT_SECS: u64 = 25;

/// The two settings tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Connection,
    Site,
}

/// Arm a hard timeout for a backend operation.
///
/// Returns a generation token; only the caller of the current token may
/// update the busy/message signals, so a stale (late-arriving) result or a
/// dropped future can never leave the UI stuck. The timer is fire-and-
/// forget: a late firing sees the stale token and does nothing.
fn arm_operation_timeout(
    generation: RwSignal<u64>,
    busy: RwSignal<bool>,
    message: RwSignal<Option<(bool, String)>>,
) -> u64 {
    let token = generation.get_untracked() + 1;
    generation.set(token);
    set_timeout(
        move || {
            if generation.get_untracked() == token {
                busy.set(false);
                message.set(Some((
                    false,
                    "การดำเนินการใช้เวลานานเกินไป (เกิน 25 วินาที) - ตรวจสอบ Host/Port/เครือข่าย แล้วลองใหม่"
                        .to_string(),
                )));
            }
        },
        std::time::Duration::from_secs(OPERATION_TIMEOUT_SECS),
    );
    token
}

#[component]
pub fn SettingsModal(state: AppState) -> impl IntoView {
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
    let generation = RwSignal::new(0u64);

    // Load the saved connection values, site settings, and current
    // medication list on mount so re-opening the dialog shows what was set
    // (the password is never stored, so it stays empty).
    spawn_local(async move {
        if let Ok(info) = api::get_connection().await {
            host.set(info.host);
            port.set(info.port.to_string());
            database.set(info.database);
            user.set(info.user);
        }
    });
    spawn_local(async move {
        match api::get_site_settings().await {
            Ok(settings) => {
                site_name.set(settings.site_name);
                history_days.set(settings.history_days);
                state.default_history_days.set(settings.history_days);
            }
            Err(e) => settings_message.set(Some((false, e.message))),
        }
    });

    /// Zeroizes an operator-typed field before dropping it - plain
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
            .map_err(|_| "พอร์ตไม่ถูกต้อง".to_string())?;
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
            return Err("กรอก Site name, Host, Database, User ให้ครบ".to_string());
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
        let token = arm_operation_timeout(generation, conn_busy, conn_message);
        spawn_local(async move {
            let result = api::test_connection(Some(&input)).await;
            if generation.get_untracked() == token {
                match result {
                    Ok(test) => conn_message.set(Some((
                        true,
                        format!("เชื่อมต่อได้ (latency {} ms)", test.latency_ms),
                    ))),
                    Err(error) => conn_message.set(Some((false, error.message))),
                }
                conn_busy.set(false);
            }
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
        let token = arm_operation_timeout(generation, conn_busy, conn_message);
        spawn_local(async move {
            let result = api::save_connection(&input).await;
            if generation.get_untracked() == token {
                match result {
                    Ok(()) => {
                        // Fields keep their values so the operator can see
                        // what is saved; they are only wiped on close.
                        conn_message.set(Some((true, "บันทึกการตั้งค่าและเชื่อมต่อแล้ว".to_string())));
                        state.configured.set(true);
                        state.health.set(ConnectionHealth::Connected);
                    }
                    Err(error) => conn_message.set(Some((false, error.message))),
                }
                conn_busy.set(false);
            }
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
        let token = arm_operation_timeout(generation, settings_busy, settings_message);
        spawn_local(async move {
            let result = api::save_site_settings(&settings).await;
            if generation.get_untracked() == token {
                match result {
                    Ok(()) => {
                        state.default_history_days.set(settings.history_days);
                        state.site_name.set(settings.site_name);
                        settings_message.set(Some((true, "บันทึกการตั้งค่าและเชื่อมต่อแล้ว".to_string())))
                    }
                    Err(e) => settings_message.set(Some((false, e.message))),
                }
                settings_busy.set(false);
            }
        });
    };

    let connection_panel = move || {
        view! {
            <section class="form-section">
                <h3 class="form-section__title">
                    <IconPlug class="icon" />
                    "การเชื่อมต่อ HOSxP"
                </h3>

                <div class="form-field">
                    <label for="cfg-host">"Host"</label>
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
                        <label for="cfg-port">"Port"</label>
                        <input
                            id="cfg-port"
                            class="form-input form-input--mono"
                            prop:value=move || port.get()
                            on:input=move |ev| port.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="form-field form-field--grow">
                        <label for="cfg-database">"Database"</label>
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
                    <label for="cfg-user">"User"</label>
                    <input
                        id="cfg-user"
                        class="form-input form-input--mono"
                        placeholder="recon_ro"
                        prop:value=move || user.get()
                        on:input=move |ev| user.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
                    <label for="cfg-password">"Password"</label>
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
                        {move || if conn_busy.get() { "กำลังทดสอบ…" } else { "ทดสอบ" }}
                    </button>
                    <button
                        class="button-primary button-primary--inline"
                        on:click=move |_| run_save_connection()
                        prop:disabled=move || conn_busy.get()
                    >
                        <IconSave class="icon" />
                        {move || if conn_busy.get() { "กำลังบันทึก…" } else { "บันทึก" }}
                    </button>
                </div>

            </section>
        }
        .into_any()
    };

    let site_panel = move || {
        view! {
            <section class="form-section">
                <h3 class="form-section__title">
                    <IconCheckCircle class="icon" />
                    "การตั้งค่าอื่นๆ"
                </h3>

                <div class="form-field">
                    <label for="cfg-site">"ชื่อสถานบริการ"</label>
                    <input
                        id="cfg-site"
                        class="form-input"
                        placeholder="เช่น โรงพยาบาลสมมติ (แสดงในรายงาน)"
                        prop:value=move || site_name.get()
                        on:input=move |ev| site_name.set(event_target_value(&ev))
                    />
                </div>
                <div class="form-field">
                    <label for="cfg-window">"ค้นประวัติย้อนหลัง (วัน)"</label>
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

                <p class="modal__note">"เฉพาะยาที่ตั้งค่าไว้เท่านั้นจะแสดงในหัวข้อ ยาที่ผู้ป่วยเคยได้รับ - ยาที่ไม่ตั้งค่า (แม้เพิ่งได้รับ) จะถือว่าเป็นยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ)"</p>
                <CurrentMedsPanel selected=selected_meds/>

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
                        "ปิด"
                    </button>
                    <button
                        class="button-primary button-primary--inline"
                        on:click=move |_| run_save_settings()
                        prop:disabled=move || settings_busy.get()
                    >
                        <IconSave class="icon" />
                        {move || if settings_busy.get() { "กำลังบันทึก…" } else { "บันทึกการตั้งค่า" }}
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
                <h2 class="modal__title">"ตั้งค่า HOSxP"</h2>
                <p class="modal__status">
                    {move || {
                        if state.configured.get() {
                            "เชื่อมต่อแล้ว"
                        } else {
                            "ยังไม่ได้ตั้งค่า"
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
                        "การเชื่อมต่อ"
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
                        "ตั้งค่าอื่นๆ"
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
/// section's save button - this panel only loads and edits.
#[component]
fn CurrentMedsPanel(selected: RwSignal<Vec<DrugInfo>>) -> impl IntoView {
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
                    placeholder="ค้นหาชื่อยา…"
                    prop:value=move || query.get()
                    on:input=move |ev| on_input(event_target_value(&ev))
                />
            </div>

            {move || {
                if searching.get() {
                    view! { <p class="modal__note">"…"</p> }.into_any()
                } else if searched.get() && results.get().is_empty() {
                    view! { <p class="modal__note">"ไม่พบรายการยา"</p> }.into_any()
                } else if !results.get().is_empty() {
                    let count = results.get().len();
                    view! {
                        <>
                            <p class="modal__note">
                                {format!("ผลการค้นหา ({count})")}
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
                                                "เพิ่ม"
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
                {move || format!("ยาที่ตั้งค่าไว้ ({})", selected.get().len())}
            </p>
            <ul class="result-list">
                {move || {
                    let list = selected.get();
                    if list.is_empty() {
                        view! { <p class="modal__note">"ยังไม่ได้ตั้งค่ายา - ยาทั้งหมดจะถือว่าหยุดใช้แล้ว"</p> }.into_any()
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
                                        "ลบ"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, strength: Option<&str>, units: Option<&str>) -> DrugInfo {
        DrugInfo {
            icode: "P1".into(),
            name: name.into(),
            strength: strength.map(str::to_string),
            units: units.map(str::to_string),
        }
    }

    #[test]
    fn drug_label_joins_name_strength_units() {
        assert_eq!(
            drug_label(&info("Paracetamol", Some("500 mg"), Some("เม็ด"))),
            "Paracetamol · 500 mg เม็ด"
        );
        assert_eq!(
            drug_label(&info("Metformin", Some("500 mg"), None)),
            "Metformin · 500 mg"
        );
        assert_eq!(
            drug_label(&info("Metformin", None, Some("เม็ด"))),
            "Metformin เม็ด"
        );
        assert_eq!(drug_label(&info("Metformin", None, None)), "Metformin");
    }
}
