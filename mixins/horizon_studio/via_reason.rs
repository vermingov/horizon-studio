fn horizon_studio_via_reason(e: &horizon_protocol::Error) -> core::ops::ControlFlow<String> {
    // ViaHorizon's own reasons (not signed in, its sign-in failing, ViaProxy
    // not running) are shown as they are written, not as an "Internal
    // Exception".
    let message = e.to_string();
    if message.starts_with("ViaHorizon") || message == crate::net::NOT_SIGNED_IN {
        return core::ops::ControlFlow::Break(message);
    }
    core::ops::ControlFlow::Continue(())
}
