fn horizon_studio_via_srv(name: &str) -> core::ops::ControlFlow<Option<(String, u16)>> {
    // A routed host's lookup points at the local ViaProxy.
    match crate::hstudio_via_net::redirect(name) {
        Some(target) => core::ops::ControlFlow::Break(Some(target)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
