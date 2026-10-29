// Each frame, with the event loop: the scripts' frame, ticks, timers and replies.
fn horizon_studio_frame(game: &mut crate::game::Game<'_>, event_loop: &winit::event_loop::ActiveEventLoop) {
    crate::game::hstudio_bridge::frame(game, event_loop);
}
