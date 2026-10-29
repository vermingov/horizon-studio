// Keys and mouse buttons for the scripts' on("key") handlers.
fn horizon_studio_key(key: &str, pressed: bool) {
    crate::game::hstudio_bridge::key(key, pressed);
}
