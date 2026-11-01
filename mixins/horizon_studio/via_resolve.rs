fn horizon_studio_via_resolve(host: &str, port: u16) -> core::ops::ControlFlow<Result<Resolved>> {
    // A routed host connects to the local ViaProxy, and keeps its own name
    // and port for the login handshake: that is where ViaProxy reads the
    // server and version from (vanilla's handshake would carry the redirect
    // target's, 127.0.0.1).
    if !crate::hstudio_via_net::routed(host) {
        return core::ops::ControlFlow::Continue(());
    }
    let resolved = match crate::hstudio_via_net::proxy_port(std::time::Duration::from_secs(10)) {
        Ok(proxy) => Ok(Resolved {
            host: host.to_owned(),
            port,
            addresses: vec![std::net::SocketAddr::from(([127, 0, 0, 1], proxy))],
        }),
        Err(why) => Err(Error::new(format!("ViaHorizon: {why}"))),
    };
    core::ops::ControlFlow::Break(resolved)
}
