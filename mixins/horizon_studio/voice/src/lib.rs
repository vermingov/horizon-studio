//! Horizon Studio's voice chat: a client for Simple Voice Chat
//! (henkelmax's mod for Fabric and NeoForge, and its Paper plugin), so a
//! player on Horizon talks to and hears the players of any server that runs
//! it, compatibility versions 20 (SVC 2.6.x) and 21 (2.7.x).
//!
//! - [`proto`]: the payloads on the Minecraft connection and the voice packets;
//! - [`crypt`]: their AES-128-GCM and framing;
//! - [`opus`]: Opus (rusty-opus 1.0.2, vendored);
//! - [`jitter`]: each speaker's packets in order, loss concealed;
//! - [`spatial`]: distance and stereo side, as OpenAL plays them;
//! - [`mixer`], [`mic`], [`resample`], [`audio`]: the speakers and the microphone;
//! - [`net`]: the voice connection's thread;
//! - [`client`]: the voice chat on one server, which the game drives.
//!
//! The game's side (Horizon's packets, its players, the HUD and the
//! settings) is Horizon Studio's support modules, which use [`VoiceChat`].

#[rustfmt::skip]
#[allow(clippy::all, clippy::pedantic, clippy::nursery, dead_code, unused, missing_docs)]
pub mod opus;

pub mod audio;
pub mod client;
pub mod crypt;
pub mod jitter;
pub mod mic;
pub mod mixer;
pub mod net;
pub mod proto;
pub mod resample;
pub mod spatial;

pub use client::{Info, Payload, ServerHost, Settings, Speaking, State, VoiceChat};
pub use mic::Activation;
pub use net::World;
pub use spatial::Listener;
