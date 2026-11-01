//! One speaker's packets in order, and their decoding with loss concealed,
//! as SVC's receiver does (`AudioPacketBuffer`, `AudioChannel`): packets
//! that arrive out of order wait (up to [`PACKET_THRESHOLD`] of them) for
//! the ones before; one older than the last played is dropped; an empty one
//! ends the speech (the decoder and the order start over); and for a gap of
//! `n` lost packets the decoder conceals `n - 1` frames, recovers the last
//! one from the next packet's forward error correction, then decodes that
//! packet. A gap longer than [`OUTPUT_BUFFER_SIZE`] starts over instead.

use crate::opus::OpusDecoder;
use crate::opus::repacketizer::{nb_frames, samples_per_frame};
use crate::proto::{FRAME_SIZE, SAMPLE_RATE};

/// `audio_packet_threshold`: packets held for the ones before them.
pub const PACKET_THRESHOLD: usize = 3;
/// `output_buffer_size`: the longest gap concealed.
pub const OUTPUT_BUFFER_SIZE: i64 = 5;

/// A packet with a place in its speaker's sequence.
pub trait Sequenced {
    fn seq(&self) -> i64;
    /// No audio: the speaker stopped.
    fn is_end(&self) -> bool;
}

/// `AudioPacketBuffer`: puts a speaker's packets in order.
pub struct JitterBuffer<T> {
    threshold: usize,
    held: Vec<T>,
    last: i64,
    flushing: bool,
}

impl<T: Sequenced> JitterBuffer<T> {
    pub fn new(threshold: usize) -> Self {
        JitterBuffer {
            threshold,
            held: Vec::new(),
            last: -1,
            flushing: false,
        }
    }

    /// How many packets wait for the ones before them.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    pub fn clear(&mut self) {
        self.held.clear();
        self.last = -1;
        self.flushing = false;
    }

    /// A packet arrived: the packets that are now ready, in the order to play them.
    pub fn push(&mut self, packet: T) -> Vec<T> {
        let mut out = Vec::new();
        if self.threshold == 0 {
            out.push(packet);
            return out;
        }
        self.drain(&mut out);
        if packet.seq() == self.last + 1 || self.last < 0 {
            self.last = packet.seq();
            out.push(packet);
        } else {
            if packet.is_end() {
                self.flushing = true;
            }
            self.held.push(packet);
            self.held.sort_by_key(Sequenced::seq);
        }
        self.drain(&mut out);
        out
    }

    fn drain(&mut self, out: &mut Vec<T>) {
        while let Some(p) = self.next() {
            out.push(p);
        }
    }

    fn next(&mut self) -> Option<T> {
        if self.flushing {
            if self.held.is_empty() {
                self.flushing = false;
                return None;
            }
            return Some(self.first());
        }
        if self.held.len() > self.threshold {
            return Some(self.first());
        }
        if self
            .held
            .first()
            .is_some_and(|p| p.seq() == self.last + 1 || self.last < 0)
        {
            return Some(self.first());
        }
        None
    }

    fn first(&mut self) -> T {
        let p = self.held.remove(0);
        self.last = p.seq();
        p
    }
}

/// What decodes a speaker's frames: Opus, or a recording one in tests.
pub trait FrameDecoder {
    /// A packet's frame.
    fn decode(&mut self, data: &[u8]) -> Vec<f32>;
    /// A lost frame, concealed.
    fn conceal(&mut self) -> Vec<f32>;
    /// A lost frame, recovered from the next packet's redundancy (else concealed).
    fn recover(&mut self, next: &[u8]) -> Vec<f32>;
    fn reset(&mut self);
}

/// Opus at 48 kHz mono. A frame that fails to decode is silence (SVC's
/// decoder throws, and its channel carries on).
pub struct Opus {
    decoder: OpusDecoder,
    out: Vec<f32>,
}

impl Opus {
    pub fn new() -> Opus {
        Opus {
            decoder: OpusDecoder::new(SAMPLE_RATE as i32, 1).expect("48 kHz mono is a valid decoder"),
            out: vec![0.0; FRAME_SIZE * 6],
        }
    }
    fn run(&mut self, data: &[u8], fec: bool) -> Vec<f32> {
        // A packet's own length (2.5 to 120 ms); a lost or recovered frame is 20 ms.
        let frame = if data.is_empty() || fec {
            FRAME_SIZE
        } else {
            let per = samples_per_frame(data[0], SAMPLE_RATE as i32).max(0) as usize;
            let n = nb_frames(data).unwrap_or(1).max(1) as usize;
            (per * n).clamp(1, self.out.len())
        };
        let result = if fec {
            self.decoder.decode_fec(data, frame, &mut self.out)
        } else {
            self.decoder.decode(data, frame, &mut self.out)
        };
        match result {
            Ok(n) => self.out[..n.min(self.out.len())].to_vec(),
            Err(_) => vec![0.0; FRAME_SIZE],
        }
    }
}

impl Default for Opus {
    fn default() -> Self {
        Opus::new()
    }
}

impl FrameDecoder for Opus {
    fn decode(&mut self, data: &[u8]) -> Vec<f32> {
        self.run(data, false)
    }
    fn conceal(&mut self) -> Vec<f32> {
        self.run(&[], false)
    }
    fn recover(&mut self, next: &[u8]) -> Vec<f32> {
        self.run(next, true)
    }
    fn reset(&mut self) {
        *self = Opus::new();
    }
}

/// What a speaker's packets come to.
#[derive(Debug, PartialEq)]
pub enum Played<M> {
    /// Frames to play, with the packet they came with.
    Frames(Vec<Vec<f32>>, M),
    /// The speaker stopped.
    End(M),
}

/// A speaker's packet: its order, its Opus data, and what it says about
/// where it is heard (`M`).
#[derive(Clone, Debug)]
pub struct Packet<M> {
    pub seq: i64,
    pub data: Vec<u8>,
    pub meta: M,
}

impl<M> Sequenced for Packet<M> {
    fn seq(&self) -> i64 {
        self.seq
    }
    fn is_end(&self) -> bool {
        self.data.is_empty()
    }
}

/// `AudioChannel`: one speaker's buffer and decoder.
pub struct Speaker<D, M> {
    buffer: JitterBuffer<Packet<M>>,
    decoder: D,
    last: i64,
    /// Packets lost so far.
    pub lost: u64,
}

impl<D: FrameDecoder, M> Speaker<D, M> {
    pub fn new(decoder: D) -> Self {
        Speaker {
            buffer: JitterBuffer::new(PACKET_THRESHOLD),
            decoder,
            last: -1,
            lost: 0,
        }
    }

    pub fn decoder(&self) -> &D {
        &self.decoder
    }

    /// A packet arrived: what to play now.
    pub fn push(&mut self, packet: Packet<M>) -> Vec<Played<M>> {
        let mut out = Vec::new();
        for p in self.buffer.push(packet) {
            // Raw PCM (a client-side channel's) has a negative sequence and no order.
            let raw = p.seq < 0;
            if !raw && self.last >= 0 && p.seq <= self.last {
                continue;
            }
            if p.data.is_empty() {
                self.last = -1;
                self.buffer.clear();
                self.decoder.reset();
                out.push(Played::End(p.meta));
                continue;
            }
            if raw {
                let pcm = p
                    .data
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.0)
                    .collect();
                out.push(Played::Frames(vec![pcm], p.meta));
                continue;
            }
            let mut missing = 0;
            if self.last >= 0 {
                missing = p.seq - (self.last + 1);
                if missing > 0 {
                    self.lost += missing as u64;
                }
                if missing > OUTPUT_BUFFER_SIZE {
                    missing = 0;
                    self.decoder.reset();
                }
            }
            self.last = p.seq;
            let mut frames = Vec::new();
            for _ in 1..missing {
                frames.push(self.decoder.conceal());
            }
            if missing > 0 {
                frames.push(self.decoder.recover(&p.data));
            }
            frames.push(self.decoder.decode(&p.data));
            out.push(Played::Frames(frames, p.meta));
        }
        out
    }
}
