//! Patient search — debounced sidebar input with auto-detected input kind
//! (CID / HN / name) and a result list.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::IconSearch;
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use med_recon_core::{PatientSummary, QueryKind, detect_query_kind};

/// Debounce window for the search input (milliseconds).
const DEBOUNCE_MS: u64 = 250;

#[component]
pub fn PatientSearch(state: AppState) -> impl IntoView {
    let query = state.search_query;
    let results = RwSignal::new(Vec::<PatientSummary>::new());
    let searching = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let searched = RwSignal::new(false);
    let lang = state.lang;

    let run_search = move || {
        let q = query.get_untracked().trim().to_string();
        if q.len() < 2 {
            results.set(Vec::new());
            searched.set(false);
            return;
        }
        searching.set(true);
        error.set(None);
        spawn_local(async move {
            match api::search_patients(&q).await {
                Ok(list) => results.set(list),
                Err(e) => {
                    results.set(Vec::new());
                    error.set(Some(e.message));
                }
            }
            searched.set(true);
            searching.set(false);
        });
    };

    // Debounce: each keystroke cancels the previous timer before scheduling
    // a new one, so the search fires 250 ms after the last keystroke.
    let last_timeout = RwSignal::new(None::<leptos::prelude::TimeoutHandle>);
    let run_search = move || run_search();
    let on_input = move |value: String| {
        query.set(value);
        if let Some(prev) = last_timeout.get() {
            prev.clear();
        }
        last_timeout.set(Some(
            set_timeout_with_handle(run_search, std::time::Duration::from_millis(DEBOUNCE_MS))
                .expect("invariant: setTimeout is available in the Tauri webview"),
        ));
    };

    let hint = move || {
        let kind = detect_query_kind(&query.get());
        match kind {
            QueryKind::Cid => tr(lang.get(), "search.hint.cid"),
            QueryKind::Hn => tr(lang.get(), "search.hint.hn"),
            QueryKind::Name => tr(lang.get(), "search.hint.name"),
        }
    };

    let on_pick = move |patient: PatientSummary| {
        let hn = patient.hn.clone();
        let name = patient.display_name();
        // Close the dropdown: clear the result list, cancel any pending
        // debounced search, and reflect the picked name in the input so only
        // the selected patient remains.
        results.set(Vec::new());
        searched.set(false);
        error.set(None);
        if let Some(prev) = last_timeout.get() {
            prev.clear();
        }
        query.set(name);
        state.patient.set(Some(patient));
        state.patient_photo.set(None);
        state.history.set(None);
        state.history_error.set(None);
        state.history_loading.set(true);
        // Each new patient starts at the configured default window. This is a
        // programmatic reset (no window_epoch bump) so the HistoryCanvas
        // re-fetch effect stays dormant and the patient-search fetch below is
        // the only in-flight request.
        state.history_days_override.set(None);
        let hn_photo = hn.clone();
        let override_days = state.history_days_override.get_untracked();
        spawn_local(async move {
            match api::load_history(&hn, override_days).await {
                Ok(history) => {
                    state.history.set(Some(history));
                    state.history_error.set(None);
                }
                Err(e) => {
                    state.history.set(None);
                    state.history_error.set(Some(e.message));
                }
            }
            state.history_loading.set(false);
        });
        // Photo is decorative — a failure just keeps the placeholder avatar.
        spawn_local(async move {
            if let Ok(Some(photo)) = api::load_patient_image(&hn_photo).await {
                state.patient_photo.set(Some(photo));
            }
        });
    };

    view! {
        <div class="sidebar__section">
            <p class="sidebar__label sidebar__label--tagline">
                <IconSearch class="icon" />
                {move || tr(lang.get(), "app.tagline")}
            </p>
            <div class="search-wrapper">
                <IconSearch class="search-icon" />
                <input
                    class="search-input"
                    placeholder={move || tr(lang.get(), "search.placeholder")}
                    autofocus
                    prop:value=move || query.get()
                    on:input=move |ev| on_input(event_target_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            run_search();
                        }
                    }
                />
            </div>
            <p class="sidebar__empty">{hint}</p>
            {move || {
                error.get().map(|msg| view! {
                    <div class="banner-warning" style="margin-top:6px">{msg}</div>
                })
            }}
            {move || {
                if searching.get() {
                    view! { <p class="sidebar__empty">"…"</p> }.into_any()
                } else if searched.get() && results.get().is_empty() {
                    view! { <p class="sidebar__empty">{tr(lang.get(), "search.no_results")}</p> }.into_any()
                } else {
                    view! { <span hidden></span> }.into_any()
                }
            }}
            {move || {
                let count = results.get().len();
                if count > 0 {
                    view! {
                        <>
                            <p class="sidebar__label">
                                {tr_f(lang.get(), "search.results", &[("n", &count.to_string())])}
                            </p>
                            <ul class="result-list">
                                {results.get().iter().map(|p| {
                                    let patient = p.clone();
                                    let name = p.display_name();
                                    let hn = p.hn.clone();
                                    let cid = p.cid.clone();
                                    view! {
                                        <li class="search-result-row" on:click=move |_| on_pick(patient.clone())>
                                            <span class="search-result-row__name">{name}</span>
                                            <span class="search-result-row__code">
                                                {move || cid.as_ref().map(|c| format!("{c} · ")).unwrap_or_default()}
                                                {hn.clone()}
                                            </span>
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
        </div>
    }
}
