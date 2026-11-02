// The tags the server sent in the configuration (hstudio_tags, in the
// protocol crate): the game keeps none but the dialogs', and saveinstance
// writes them into its world's data pack. The tags_in hook hands each
// ClientboundUpdateTagsPacket here; the last one is kept, each holding
// every registry's tags.

use crate::packets::configuration::{RegistryTags, UpdateTags};
use std::sync::Mutex;

static LAST: Mutex<Vec<RegistryTags>> = Mutex::new(Vec::new());

/// The tags of an UpdateTags packet, in place of the last ones.
pub fn keep(packet: &UpdateTags) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = packet.registries.clone();
}

/// The tags the server sent last.
pub fn last() -> Vec<RegistryTags> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
