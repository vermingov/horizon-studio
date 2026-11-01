//! Linear resampling between the devices' rates and SVC's 48 kHz, as a
//! stream (any block sizes in, the same result).

/// A mono stream from `from` Hz to `to` Hz.
#[derive(Clone, Debug)]
pub struct Resampler {
    step: f64,
    /// Where the next output sample is, from `prev` (0) towards the next input sample (1).
    pos: f64,
    prev: f32,
    primed: bool,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Resampler {
        Resampler {
            step: f64::from(from.max(1)) / f64::from(to.max(1)),
            pos: 0.0,
            prev: 0.0,
            primed: false,
        }
    }

    pub fn passthrough(&self) -> bool {
        (self.step - 1.0).abs() < 1e-12
    }

    /// Appends `input`, resampled, to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough() {
            out.extend_from_slice(input);
            return;
        }
        for &x in input {
            if !self.primed {
                self.prev = x;
                self.primed = true;
                continue;
            }
            while self.pos < 1.0 {
                out.push(self.prev + (x - self.prev) * self.pos as f32);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = x;
        }
    }
}

/// Interleaved frames of any channel count, mixed down to mono.
pub fn to_mono(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    let channels = channels.max(1);
    if channels == 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    for frame in interleaved.chunks_exact(channels) {
        out.push(frame.iter().sum::<f32>() / channels as f32);
    }
}
