//! Sidebar PMH card - the patient's past medical history
//! (`opdscreen.pmh`), rendered as a compact card below the profile card.
//!
//! The value is the latest record for the patient (newest `vstdate` wins,
//! backend-side); it is cumulative history and not bounded by the history
//! window. Free text, possibly multi-line - shown verbatim.

use leptos::prelude::*;

use crate::components::icons::IconClipboard;
use crate::state::AppState;

#[component]
pub fn PmhCard(state: AppState) -> impl IntoView {
    view! {
        {move || {
            // Only appear once a patient is picked and the history has
            // actually loaded - hiding while the load is in flight avoids a
            // misleading flash of "ไม่มีข้อมูล" before the data arrives.
            if state.patient.get().is_none() || state.history_loading.get() {
                return None;
            }
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
                                "ประวัติการเจ็บป่วย (PMH)"
                            </p>
                        </div>
                        {move || match pmh.as_ref() {
                            Some(text) => view! {
                                <p class="pmh-card__text">{text.clone()}</p>
                            }.into_any(),
                            None => view! {
                                <p class="pmh-card__empty">"ไม่มีข้อมูล"</p>
                            }.into_any(),
                        }}
                    </div>
                </div>
            })
        }}
    }
}
