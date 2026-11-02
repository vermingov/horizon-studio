// The server's tags in the configuration, kept for saveinstance's data pack.
fn horizon_studio_tags_in(packet: &crate::packets::configuration::UpdateTags) {
    crate::hstudio_tags::keep(packet);
}
