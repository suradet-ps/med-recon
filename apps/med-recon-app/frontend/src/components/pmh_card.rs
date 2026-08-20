//! Sidebar PMH card — the patient's past medical history
//! (`opdscreen.pmh`), rendered as a compact card below the profile card.
//!
//! The value is the latest record for the patient (newest `vstdate` wins,
//! backend-side); it is cumulative history and not bounded by the history
//! window. Free text, possibly multi-line — shown verbatim.

use leptos::prelude::*;

use crate::components::icons::IconClipboard;
use crate::i18n::tr;
use crate::state::AppState;

#[component]
pub fn PmhCard(state: AppState) -> impl IntoView {
    let lang = state.lang;
    view! {
        {move || {
            let pmh = state
                .history
                .get()
                .and_then(|h| h.pmh.clone())
                .filter(|t| !t.trim().is_empty());
            Some(view! {
                <div class="sidebar__section sidebar__section--patient">
                    <div class="patient-card">
                        <div class="patient-card__head">
                            <p class="sidebar__label patient-card__label">
                                <IconClipboard class="icon" />
                                {tr(lang.get(), "pmh.title")}
                            </p>
                        </div>
                        {move || match pmh.as_ref() {
                            Some(text) => view! {
                                <p class="pmh-card__text">{text.clone()}</p>
                            }.into_any(),
                            None => view! {
                                <p class="pmh-card__empty">{tr(lang.get(), "pmh.empty")}</p>
                            }.into_any(),
                        }}
                    </div>
                </div>
            })
        }}
    }
}
