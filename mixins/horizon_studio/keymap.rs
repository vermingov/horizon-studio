// The game's first step with each window event: while the studio is open,
// it takes the keyboard and the mouse (queued for the frame hook).
fn horizon_studio_window_input(event: &mut winit::event::WindowEvent) {
    crate::game::hstudio_bridge::window_input(event);
}
