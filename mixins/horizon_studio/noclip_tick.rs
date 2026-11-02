// The player passes through blocks while a Roblox noclip script runs, in a
// world played alone too (whose tick puts the game mode's physics back).
fn horizon_studio_noclip_tick(world: &mut MotionWorld<'_, '_, '_>) {
    if crate::mobs::hstudio_body::noclip() {
        world.player.spectator = true;
    }
}
