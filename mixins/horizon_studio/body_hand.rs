// The first-person arm, unless Roblox scripts took it off the local character.
fn horizon_studio_body_hand(
    game: &crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
    stack: Option<&horizon_client::items::ItemStack>,
    canceled: &mut bool,
) {
    if stack.is_none() && !crate::game::hstudio_bridge::arm_shown(game, hand) {
        *canceled = true;
    }
}
