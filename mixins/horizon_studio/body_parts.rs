// The parts of the player's own model Roblox scripts took off: hidden.
fn horizon_studio_body_parts(value: (), p: &mut Poses, hidden: u8) {
    crate::mobs::hstudio_body::hide(p, hidden);
    value
}
