// Natrium in Video Settings, as Sodium is in Java's: a section of its own after
// Preferences, its switches as vanilla's buttons two to a row, and what it has
// skipped under them. Each button is a button of Natrium's Options > Mods page
// (`Act::NativeButton`), so a click reaches Natrium as one there does, and the
// section shows the switches as they are each frame (`horizon_core::natrium`).

use super::*;
use horizon_core::natrium as flags;
use std::cell::RefCell;

/// Video Settings' last section, Preferences: Natrium's follows it.
const PREFERENCES: [Opt; 4] = [Opt::Autosave, Opt::Vignette, Opt::AttackIndicator, Opt::ChunkFade];

thread_local! {
    /// The settings search's text, as the options screens last had it.
    static SEARCH: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Keeps the settings search's text for the section.
pub(super) fn searching(ui: &OptionsUi) {
    SEARCH.with(|search| {
        if *search.borrow() != ui.settings_search.value {
            search.replace(ui.settings_search.value.clone());
        }
    });
}

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

/// Natrium's buttons, each with its label: Natrium's switch and the preset,
/// then the culling switches, then block faces and ticks.
fn buttons(index: usize, switches: flags::Switches) -> Vec<(String, Widget)> {
    let on = switches[0];
    let widget = |b: usize, label: String, live: bool| {
        let w = button(
            0.0,
            0.0,
            150.0,
            Text::literal(label.clone()),
            Act::NativeButton(index, b),
        );
        (label, if live { w } else { inactive(w) })
    };
    let switch = |b: usize| {
        let label = format!("{}: {}", flags::CAPTIONS[b], on_off(switches[b]));
        widget(b, label, b == 0 || on)
    };
    vec![
        switch(0),
        widget(flags::PRESET, flags::PRESET_LABEL.to_owned(), on),
        switch(1),
        switch(2),
        switch(4),
        switch(3),
    ]
}

/// Video Settings' Preferences in pairs (as `pairs`), then Natrium's
/// section; `None` for any other options, or while Natrium is off in
/// Options > Mods.
pub(super) fn preferences(opts: &[Opt]) -> Option<Vec<Row>> {
    if opts != PREFERENCES {
        return None;
    }
    let (index, switches) = flags::video()?;
    let mut rows: Vec<Row> = opts.chunks(2).map(|c| Row::Pair(c[0], c.get(1).copied())).collect();
    // While searching, the buttons the search names (the search keeps every
    // row of buttons, and no heading over them).
    let query = SEARCH.with(|search| search.borrow().to_lowercase());
    let mut shown = buttons(index, switches)
        .into_iter()
        .filter(|(label, _)| query.is_empty() || label.to_lowercase().contains(&query))
        .map(|(_, w)| w);
    if query.is_empty() {
        rows.push(Row::Header("Natrium"));
    }
    while let Some(a) = shown.next() {
        rows.push(Row::Buttons(a, shown.next()));
    }
    if query.is_empty() {
        rows.push(Row::Note(flags::skipped()));
    }
    Some(rows)
}
