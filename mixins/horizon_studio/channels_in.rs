// The configuration's custom payloads: the plugin channels the server names.
fn horizon_studio_channels_in(payload: &horizon_protocol::packets::configuration::CustomPayload) {
    crate::net::hstudio_channels::heard(&payload.channel, &payload.data);
}
