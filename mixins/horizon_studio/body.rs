// The parts of the player's own model the studio hides (Roblox scripts took
// them off the local character, hstudio_bridge::set_body), in the game's
// library: the model's poses read them as they are set up
// (player_model_visibility). Only the player's own model is marked (OWN, in
// its hidden_model_parts, whose top bit the game leaves clear).

use std::sync::atomic::{AtomicU16, Ordering};

/// The parts the studio can hide, in the mask's order (hstudio::BODY_PARTS).
pub const PARTS: [&str; 12] = [
    "head",
    "hat",
    "body",
    "jacket",
    "left_arm",
    "right_arm",
    "left_sleeve",
    "right_sleeve",
    "left_leg",
    "right_leg",
    "left_pants",
    "right_pants",
];

/// The mark of the player's own model, in its hidden_model_parts.
pub const OWN: u8 = 0x80;

static HIDDEN: AtomicU16 = AtomicU16::new(0);

/// The parts hidden now, as a mask over PARTS.
pub fn set(mask: u16) {
    HIDDEN.store(mask, Ordering::Relaxed);
}

pub fn mask() -> u16 {
    HIDDEN.load(Ordering::Relaxed)
}

/// After a player model's visibility is set up: the player's own (marked
/// OWN) hides the parts the studio hides.
pub fn hide(p: &mut super::anim::Poses, hidden: u8) {
    if hidden & OWN == 0 {
        return;
    }
    let mask = mask();
    for (i, part) in PARTS.iter().enumerate() {
        if mask & (1 << i) != 0 {
            p.visible(part, false);
        }
    }
}
