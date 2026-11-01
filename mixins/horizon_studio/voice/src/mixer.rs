//! The speakers mixed to stereo at 48 kHz: the network thread queues each
//! speaker's decoded frames with their gains (distance and side), the output
//! stream takes what it plays. A speaker starts once two frames (40 ms) wait,
//! so a late packet doesn't cut the voice; one far behind loses its oldest
//! audio rather than lag.

use crate::proto::FRAME_SIZE;
use std::collections::{HashMap, VecDeque};

/// Frames a speaker waits for before it plays (and again after running dry).
pub const PREBUFFER_FRAMES: usize = 2;
/// The most a speaker keeps queued: more is dropped, oldest first.
pub const MAX_QUEUED_FRAMES: usize = 12;

#[derive(Default)]
struct Voice {
    /// Interleaved stereo samples, gains applied.
    queue: VecDeque<f32>,
    playing: bool,
    /// The gains the last frame ended on, which the next starts from.
    gains: Option<[f32; 2]>,
    /// The speaker stopped: what is queued plays out without waiting.
    ended: bool,
}

/// Every speaker's queued audio.
pub struct Mixer {
    voices: HashMap<u128, Voice>,
    /// The voice chat volume (0 to 3; 1 as it comes).
    pub volume: f32,
    /// Nothing is heard (or queued).
    pub deafened: bool,
    /// Samples (stereo frames) played so far, and how many had a voice in them.
    pub rendered: u64,
    pub voiced: u64,
}

impl Default for Mixer {
    fn default() -> Self {
        Mixer::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            voices: HashMap::new(),
            volume: 1.0,
            deafened: false,
            rendered: 0,
            voiced: 0,
        }
    }

    /// Queues a speaker's mono frame, heard with `gains` `[left, right]`
    /// (faded from the frame before's, so a moving speaker doesn't click).
    pub fn push(&mut self, channel: u128, mono: &[f32], gains: [f32; 2]) {
        if self.deafened {
            return;
        }
        let v = self.voices.entry(channel).or_default();
        v.ended = false;
        let from = v.gains.unwrap_or(gains);
        let n = mono.len().max(1) as f32;
        for (i, &s) in mono.iter().enumerate() {
            let t = (i as f32 + 1.0) / n;
            let l = from[0] + (gains[0] - from[0]) * t;
            let r = from[1] + (gains[1] - from[1]) * t;
            v.queue.push_back(s * l);
            v.queue.push_back(s * r);
        }
        v.gains = Some(gains);
        let max = MAX_QUEUED_FRAMES * FRAME_SIZE * 2;
        if v.queue.len() > max {
            let extra = v.queue.len() - max;
            v.queue.drain(..extra);
        }
    }

    /// The speaker stopped talking: what is queued plays out.
    pub fn end(&mut self, channel: u128) {
        if let Some(v) = self.voices.get_mut(&channel) {
            v.ended = true;
            v.gains = None;
        }
    }

    /// Forgets a speaker (gone from the server).
    pub fn remove(&mut self, channel: u128) {
        self.voices.remove(&channel);
    }

    pub fn clear(&mut self) {
        self.voices.clear();
    }

    /// Speakers with audio waiting.
    pub fn active(&self) -> usize {
        self.voices.values().filter(|v| !v.queue.is_empty()).count()
    }

    /// Stereo frames queued for a speaker.
    pub fn queued(&self, channel: u128) -> usize {
        self.voices.get(&channel).map_or(0, |v| v.queue.len() / 2)
    }

    /// Mixes into `out` (interleaved stereo, 48 kHz), which it overwrites.
    pub fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let frames = out.len() / 2;
        self.rendered += frames as u64;
        if self.deafened {
            self.voices.clear();
            return;
        }
        let mut any = false;
        let volume = self.volume.clamp(0.0, 3.0);
        for v in self.voices.values_mut() {
            if !v.playing {
                if v.queue.len() >= PREBUFFER_FRAMES * FRAME_SIZE * 2 || (v.ended && !v.queue.is_empty()) {
                    v.playing = true;
                } else {
                    continue;
                }
            }
            let n = v.queue.len().min(frames * 2);
            for (o, s) in out.iter_mut().zip(v.queue.drain(..n)) {
                *o += s * volume;
            }
            any |= n > 0;
            if v.queue.is_empty() {
                v.playing = false;
            }
        }
        if any {
            self.voiced += frames as u64;
        }
        for o in out.iter_mut() {
            *o = o.clamp(-1.0, 1.0);
        }
        self.voices.retain(|_, v| !(v.ended && v.queue.is_empty()));
    }
}
