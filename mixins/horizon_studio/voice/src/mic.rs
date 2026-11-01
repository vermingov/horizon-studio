//! The microphone: its samples (mono, 48 kHz) as the input stream delivers
//! them, and SVC's decision per 20 ms frame of whether to send it: push to
//! talk while the key is held (and 5 frames after, `ptt_deactivation_delay`),
//! or voice activation while the frame's peak is above the threshold (and 25
//! frames after, `voice_deactivation_delay`). The gain is in dB, as SVC's
//! microphone gain (-40 to 24), with a limiter that keeps it from clipping.

use crate::proto::FRAME_SIZE;
use std::collections::VecDeque;
use std::sync::Mutex;

/// The quietest level, in dBFS (`AudioUtils.LOWEST_DB`).
pub const LOWEST_DB: f32 = -127.0;
pub const PTT_DELAY_FRAMES: u32 = 5;
pub const VOICE_DELAY_FRAMES: u32 = 25;
pub const MIN_GAIN_DB: f32 = -40.0;
pub const MAX_GAIN_DB: f32 = 24.0;
/// At most half a second waits: older samples are dropped.
const MAX_BUFFERED: usize = 48_000 / 2;

/// How the microphone is turned on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Activation {
    /// While the push to talk key is held (SVC's default).
    #[default]
    PushToTalk,
    /// While the voice is louder than the threshold.
    Voice,
}

impl Activation {
    pub fn name(self) -> &'static str {
        match self {
            Activation::PushToTalk => "ptt",
            Activation::Voice => "voice",
        }
    }
    pub fn parse(s: &str) -> Option<Activation> {
        match s {
            "ptt" | "push_to_talk" | "push-to-talk" => Some(Activation::PushToTalk),
            "voice" | "vad" | "voice_activation" => Some(Activation::Voice),
            _ => None,
        }
    }
}

/// What the input stream has captured, waiting for the network thread.
#[derive(Default)]
pub struct MicBuffer {
    samples: Mutex<VecDeque<f32>>,
}

impl MicBuffer {
    pub fn new() -> MicBuffer {
        MicBuffer::default()
    }
    /// Mono samples at 48 kHz.
    pub fn push(&self, samples: &[f32]) {
        let mut q = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        q.extend(samples.iter().copied());
        if q.len() > MAX_BUFFERED {
            let extra = q.len() - MAX_BUFFERED;
            q.drain(..extra);
        }
    }
    /// The next whole frame, if there is one.
    pub fn frame(&self) -> Option<Vec<f32>> {
        let mut q = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() < FRAME_SIZE {
            return None;
        }
        Some(q.drain(..FRAME_SIZE).collect())
    }
    pub fn len(&self) -> usize {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn clear(&self) {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// A frame's peak level in dBFS (`AudioUtils.getHighestAudioLevel`).
pub fn peak_db(frame: &[f32]) -> f32 {
    let peak = frame.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak <= 0.0 {
        return LOWEST_DB;
    }
    (20.0 * peak.log10()).clamp(LOWEST_DB, 0.0)
}

pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Per-frame choices for the microphone.
pub struct MicProcessor {
    hold: u32,
    /// The limiter's recent largest gains (`VolumeManager`).
    limits: [f32; 32],
    at: usize,
    /// The last frame's level (after the gain), in dBFS.
    pub level_db: f32,
}

impl Default for MicProcessor {
    fn default() -> Self {
        MicProcessor::new()
    }
}

impl MicProcessor {
    pub fn new() -> MicProcessor {
        MicProcessor {
            hold: 0,
            limits: [f32::MAX; 32],
            at: 0,
            level_db: LOWEST_DB,
        }
    }

    pub fn reset(&mut self) {
        self.hold = 0;
    }

    /// Applies the gain to `frame` (limited so it doesn't clip) and decides
    /// whether to send it: `held` is the push to talk key.
    pub fn process(
        &mut self,
        frame: &mut [f32],
        activation: Activation,
        gain_db: f32,
        threshold_db: f32,
        held: bool,
        muted: bool,
    ) -> bool {
        let gain = if gain_db <= MIN_GAIN_DB {
            0.0
        } else {
            db_to_linear(gain_db.min(MAX_GAIN_DB))
        };
        let peak = frame.iter().fold(0f32, |m, s| m.max(s.abs()));
        self.limits[self.at] = if peak > 0.0 { gain.min(0.99 / peak) } else { gain };
        self.at = (self.at + 1) % self.limits.len();
        let g = self.limits.iter().copied().fold(gain, f32::min);
        for s in frame.iter_mut() {
            *s = (*s * g).clamp(-1.0, 1.0);
        }
        self.level_db = peak_db(frame);
        if muted {
            self.hold = 0;
            return false;
        }
        let (on, delay) = match activation {
            Activation::PushToTalk => (held, PTT_DELAY_FRAMES),
            Activation::Voice => (self.level_db > threshold_db, VOICE_DELAY_FRAMES),
        };
        if on {
            self.hold = delay;
            true
        } else if self.hold > 0 {
            self.hold -= 1;
            true
        } else {
            false
        }
    }
}
