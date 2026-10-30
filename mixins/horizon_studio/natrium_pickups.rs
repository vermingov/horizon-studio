fn horizon_studio_natrium_pickups(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    // Natrium: the same pickups, near items asked what they are first.
    if horizon_core::natrium::ticks() {
        natrium_villagers::villager_pickups(game);
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
