//! Lucide-style SVG icon components.
//!
//! Stroke-based icons (24×24 viewBox, `currentColor`, 2px stroke, round
//! caps) — render crisply at any size and inherit their color from the
//! surrounding text. Icons are decorative: `aria-hidden` and never carry
//! text alternatives; nearby text always explains the state.

use leptos::prelude::*;

/// Generates one icon component: an `<svg>` shell with the given nodes.
macro_rules! icon {
    ($(#[$doc:meta])* $name:ident, $($nodes:tt)*) => {
        $(#[$doc])*
        #[component]
        pub fn $name(class: &'static str) -> impl IntoView {
            view! {
                <svg
                    class=class
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                >
                    $($nodes)*
                </svg>
            }
        }
    };
}

icon!(
    /// Magnifying glass — search actions.
    IconSearch,
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.3-4.3" />
);

icon!(
    /// Gear — settings.
    IconSettings,
    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
    <circle cx="12" cy="12" r="3" />
);

icon!(
    /// Plug — test connection.
    IconPlug,
    <path d="M12 22v-5" />
    <path d="M9 8V2" />
    <path d="M15 8V2" />
    <path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z" />
);

icon!(
    /// Save (floppy disk).
    IconSave,
    <path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
    <path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" />
    <path d="M7 3v4a1 1 0 0 0 1 1h7" />
);

icon!(
    /// X — close.
    IconX,
    <path d="M18 6 6 18" />
    <path d="m6 6 12 12" />
);

icon!(
    /// Printer — export report.
    IconPrinter,
    <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2" />
    <path d="M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6" />
    <rect x="6" y="14" width="12" height="8" rx="1" />
);

icon!(
    /// Alert triangle — warnings.
    IconAlert,
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
    <path d="M12 9v4" />
    <path d="M12 17h.01" />
);

icon!(
    /// User — patient.
    IconUser,
    <path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" />
    <circle cx="12" cy="7" r="4" />
);

icon!(
    /// Check-circle — positive state (active medication).
    IconCheckCircle,
    <circle cx="12" cy="12" r="10" />
    <path d="m9 12 2 2 4-4" />
);

icon!(
    /// X-circle — negative state (lapsed / allergy).
    IconXCircle,
    <circle cx="12" cy="12" r="10" />
    <path d="m15 9-6 6" />
    <path d="m9 9 6 6" />
);

icon!(
    /// Chevron — collapsible section toggle.
    IconChevron,
    <path d="m9 18 6-6-6-6" />
);

icon!(
    /// Clipboard — screening (CC/PE) records.
    IconClipboard,
    <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
    <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
    <path d="M12 11h4" />
    <path d="M12 16h4" />
    <path d="M8 11h.01" />
    <path d="M8 16h.01" />
);

/// Brand mark — a faithful miniature of `icon-master.svg`: warm-yellow
/// disc, angled white blister board with four light-green pockets, and the
/// reconciliation badge (white disc + medium-green cross) overlapping the
/// bottom-right pocket. Colors follow the app's design tokens via CSS
/// variables.
#[component]
pub fn IconLogo(class: &'static str) -> impl IntoView {
    view! {
        <svg class=class viewBox="0 0 24 24" aria-hidden="true">
            <circle
                cx="12"
                cy="12"
                r="11.25"
                fill="var(--logo-disc, #FBC02D)"
                stroke="var(--ink)"
                stroke-width="1"
            />
            <circle
                cx="12"
                cy="12"
                r="9.6"
                fill="none"
                stroke="var(--logo-ring, #FFF8E1)"
                stroke-width="0.5"
                opacity="0.55"
            />
            <g transform="rotate(-8 12 12)">
                <rect
                    x="4.6"
                    y="4.2"
                    width="14.8"
                    height="15.6"
                    rx="2.6"
                    fill="var(--on-brand)"
                    stroke="var(--ink)"
                    stroke-width="0.8"
                />
                <circle
                    cx="8.4"
                    cy="8.6"
                    r="2.1"
                    fill="var(--logo-pocket, #66BB6A)"
                    stroke="var(--ink)"
                    stroke-width="0.55"
                />
                <circle
                    cx="15.6"
                    cy="8.6"
                    r="2.1"
                    fill="var(--logo-pocket, #66BB6A)"
                    stroke="var(--ink)"
                    stroke-width="0.55"
                />
                <circle
                    cx="8.4"
                    cy="15.6"
                    r="2.1"
                    fill="var(--logo-pocket, #66BB6A)"
                    stroke="var(--ink)"
                    stroke-width="0.55"
                />
                <circle
                    cx="15.6"
                    cy="15.6"
                    r="2.1"
                    fill="var(--logo-pocket, #66BB6A)"
                    stroke="var(--ink)"
                    stroke-width="0.55"
                />
            </g>
            <circle
                cx="16.4"
                cy="15.2"
                r="4.2"
                fill="var(--on-brand)"
                stroke="var(--logo-badge, #43A047)"
                stroke-width="1.1"
            />
            <path
                d="M16.4 13.2v4M14.4 15.2h4"
                stroke="var(--logo-cross, #2E7D32)"
                stroke-width="1.7"
                stroke-linecap="round"
            />
        </svg>
    }
}
