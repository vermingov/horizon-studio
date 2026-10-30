fn horizon_studio_via_address(text: &str) -> core::ops::ControlFlow<Option<(String, u16)>> {
    // Joins and pings ViaHorizon routes parse as ViaProxy's internal host.
    match crate::hstudio_via_net::route(text) {
        Some(host) => core::ops::ControlFlow::Break(Some((host, 25565))),
        None => core::ops::ControlFlow::Continue(()),
    }
}
