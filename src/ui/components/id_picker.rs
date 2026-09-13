//! Multi-select picker for "comma-separated IDs" admin fields.
//!
//! Pre-cycle: the Sets / Accessory Sets / Tea Sets tabs asked the
//! admin to paste comma-separated IDs of bikes, accessories, and tea
//! products into a plain `<textarea>`. Admins had to leave the form,
//! look up IDs in another tab, copy them back, and hope they spelled
//! everything correctly. User-reported as «не понимаю как добавлять».
//!
//! This component replaces that flow with a searchable checkbox list:
//! options come from the already-loaded catalog (`AdminBike`,
//! `AdminAccessory`, …), the admin sees real names, picks with
//! checkboxes, and the surrounding form receives a `Vec<String>` of
//! selected IDs without any manual string handling.

use dioxus::prelude::*;

#[derive(Clone, PartialEq, Props)]
pub struct IdPickerProps {
    /// Label shown above the picker. e.g. "Bikes" or "Accessories".
    pub label: String,
    /// Catalog of `(id, display_label)` pairs the admin can pick from.
    /// Order is preserved as the render order.
    pub options: Vec<(String, String)>,
    /// Current selection — bidirectional. The picker mutates this on
    /// each checkbox toggle; the form reads it on submit.
    pub selected: Signal<Vec<String>>,
}

#[component]
pub fn IdPicker(props: IdPickerProps) -> Element {
    let IdPickerProps {
        label,
        options,
        mut selected,
    } = props;
    let mut search = use_signal(String::new);
    let mut expanded = use_signal(|| false);

    let selected_count = selected.read().len();
    let total = options.len();
    let q = search.read().to_lowercase();
    let filtered: Vec<(String, String)> = options
        .iter()
        .filter(|(_, lbl)| q.is_empty() || lbl.to_lowercase().contains(&q))
        .cloned()
        .collect();
    let is_expanded = *expanded.read();

    rsx! {
        div {
            style: "
                background:#1a1a2e;
                border:2px solid #2a2a4a;
                border-radius:6px;
                padding:8px 10px;
                margin:4px 0;
            ",
            // Header — label, count, toggle button.
            div {
                style: "display:flex;align-items:center;gap:8px;",
                div {
                    style: "flex:1;font-size:13px;font-weight:600;color:#e8e8e8;",
                    "{label}: {selected_count}/{total}"
                }
                button {
                    style: "
                        padding:4px 10px;
                        background:#2a2a4a;
                        color:#e8e8e8;
                        border:none;
                        border-radius:4px;
                        font-size:11px;
                        cursor:pointer;
                    ",
                    onclick: move |_| expanded.set(!is_expanded),
                    if is_expanded { "Скрыть" } else { "Выбрать ▾" }
                }
            }
            if is_expanded {
                if total == 0 {
                    div {
                        style: "
                            margin-top:8px;padding:8px;
                            background:#16213e;border-radius:4px;
                            color:#8b8b9e;font-size:12px;text-align:center;
                        ",
                        "Каталог пуст — добавьте элементы во вкладке выше."
                    }
                } else {
                    // Search input.
                    input {
                        style: "
                            margin-top:8px;
                            width:100%;
                            padding:6px 8px;
                            background:#0f0f1a;
                            color:#e8e8e8;
                            border:1px solid #2a2a4a;
                            border-radius:4px;
                            font-size:13px;
                            box-sizing:border-box;
                        ",
                        placeholder: "🔍 Поиск…",
                        value: "{search}",
                        oninput: move |e| search.set(e.value()),
                    }
                    // Selected pills (clickable to deselect). A pill
                    // whose id doesn't resolve in the current catalog
                    // (e.g. the underlying bike was deleted after the
                    // set was created) is rendered with a warning style
                    // so the admin notices and can decide whether to
                    // remove the stale reference or repopulate the
                    // catalog.
                    if selected_count > 0 {
                        div {
                            style: "
                                margin-top:8px;
                                display:flex;flex-wrap:wrap;gap:4px;
                            ",
                            for sid in selected.read().clone().into_iter() {
                                {
                                    let resolved = options
                                        .iter()
                                        .find(|(id, _)| id == &sid)
                                        .map(|(_, lbl)| lbl.clone());
                                    let is_stale = resolved.is_none();
                                    let label_for_id = resolved.unwrap_or_else(|| sid.clone());
                                    let (bg, color, border, prefix, title) = if is_stale {
                                        (
                                            "#ff475715",
                                            "#ff8a92",
                                            "#ff4757",
                                            "⚠️ ",
                                            format!("ID '{}' не найден в каталоге — возможно элемент удалён", sid),
                                        )
                                    } else {
                                        (
                                            "#39ff1415",
                                            "#39ff14",
                                            "#39ff14",
                                            "",
                                            String::new(),
                                        )
                                    };
                                    let pill_style = format!(
                                        "padding:3px 8px;background:{};color:{};\
                                         border:1px solid {};border-radius:12px;\
                                         font-size:11px;cursor:pointer;",
                                        bg, color, border
                                    );
                                    let sid_for_remove = sid.clone();
                                    rsx! {
                                        button {
                                            key: "pill-{sid}",
                                            style: "{pill_style}",
                                            title: "{title}",
                                            onclick: move |_| {
                                                let mut s = selected.write();
                                                s.retain(|x| x != &sid_for_remove);
                                            },
                                            "{prefix}{label_for_id} ×"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Checkbox list.
                    div {
                        style: "
                            margin-top:8px;
                            max-height:240px;
                            overflow-y:auto;
                            display:flex;flex-direction:column;gap:2px;
                            background:#0f0f1a;
                            border:1px solid #2a2a4a;
                            border-radius:4px;
                            padding:4px;
                        ",
                        if filtered.is_empty() {
                            div {
                                style: "padding:8px;color:#8b8b9e;font-size:12px;text-align:center;",
                                "Ничего не найдено"
                            }
                        } else {
                            for (id, lbl) in filtered {
                                {
                                    let id_for_toggle = id.clone();
                                    let id_for_check = id.clone();
                                    let is_checked = selected.read().iter().any(|x| x == &id_for_check);
                                    let row_bg = if is_checked { "#16213e" } else { "transparent" };
                                    let row_style = format!(
                                        "display:flex;align-items:center;gap:8px;\
                                         padding:4px 6px;font-size:13px;color:#e8e8e8;\
                                         cursor:pointer;border-radius:3px;background:{};",
                                        row_bg
                                    );
                                    rsx! {
                                        label {
                                            key: "{id}",
                                            style: "{row_style}",
                                            input {
                                                r#type: "checkbox",
                                                checked: is_checked,
                                                onchange: move |e| {
                                                    let mut s = selected.write();
                                                    if e.checked() {
                                                        if !s.iter().any(|x| x == &id_for_toggle) {
                                                            s.push(id_for_toggle.clone());
                                                        }
                                                    } else {
                                                        s.retain(|x| x != &id_for_toggle);
                                                    }
                                                },
                                            }
                                            span { "{lbl}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
