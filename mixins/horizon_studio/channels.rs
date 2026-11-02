// The plugin channels a server names while the client configures
// (hstudio_channels, in the game's net module): its minecraft:register
// payloads and the channels it sends on, before play begins. The
// configuration runs on the network thread; the studio's
// ReplicatedStorage.Server takes them on the frame (hstudio_server_content).

use std::sync::Mutex;

/// The channels heard since the frame last took them.
static HEARD: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The most kept between two frames.
const MAX: usize = 4096;

/// The channels a `minecraft:register` payload names (NUL between them).
pub fn registered(data: &[u8]) -> Vec<String> {
    data.split(|&b| b == 0)
        .filter(|c| !c.is_empty() && c.len() <= 256)
        .filter_map(|c| std::str::from_utf8(c).ok())
        .map(str::to_owned)
        .collect()
}

/// A custom payload of the configuration: its channel, and the ones it
/// registers.
pub fn heard(channel: &str, data: &[u8]) {
    let mut h = HEARD.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if h.len() >= MAX {
        return;
    }
    if matches!(channel, "minecraft:register" | "REGISTER") {
        h.extend(registered(data));
    } else {
        h.push(channel.to_owned());
    }
}

/// The channels heard since the last time.
pub fn take() -> Vec<String> {
    std::mem::take(&mut *HEARD.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}
