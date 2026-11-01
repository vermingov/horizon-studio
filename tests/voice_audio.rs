// The voice chat's audio: Opus both ways, where a voice is heard from
// (distance and side, as OpenAL plays it), each speaker's packets in order
// with loss concealed as SVC's receiver does, the mixer, the microphone's
// push to talk and voice activation, and resampling.

use hstudio_voice::Settings;
use hstudio_voice::jitter::{FrameDecoder, JitterBuffer, OUTPUT_BUFFER_SIZE, Opus, Packet, Played, Sequenced, Speaker};
use hstudio_voice::mic::{Activation, MicBuffer, MicProcessor, PTT_DELAY_FRAMES, VOICE_DELAY_FRAMES, peak_db};
use hstudio_voice::mixer::{Mixer, PREBUFFER_FRAMES};
use hstudio_voice::net::encoder;
use hstudio_voice::proto::{Codec, FRAME_SIZE, MAX_OPUS_PAYLOAD};
use hstudio_voice::resample::{Resampler, to_mono};
use hstudio_voice::spatial::{Listener, centre, distance_gain, positional, stereo_gains};

/// A voiced sound: a 140 Hz buzz with its harmonics, in syllables.
fn voice(frames: usize) -> Vec<f32> {
    (0..frames * FRAME_SIZE)
        .map(|i| {
            let t = i as f32 / 48_000.0;
            let env = (std::f32::consts::PI * 2.5 * t).sin().abs();
            let s: f32 = (1..12)
                .map(|h| (2.0 * std::f32::consts::PI * 140.0 * h as f32 * t).sin() / h as f32)
                .sum();
            0.3 * env * s
        })
        .collect()
}

/// The best correlation of `b` with `a` delayed by up to `max` samples.
fn correlation(a: &[f32], b: &[f32], max: usize) -> (f32, usize) {
    let n = a.len().min(b.len()) - max;
    let mut best = (0.0, 0);
    for lag in 0..max {
        let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..n {
            let (x, y) = (f64::from(a[i]), f64::from(b[i + lag]));
            ab += x * y;
            aa += x * x;
            bb += y * y;
        }
        let c = (ab / (aa.sqrt() * bb.sqrt()).max(1e-12)) as f32;
        if c > best.0 {
            best = (c, lag);
        }
    }
    best
}

#[test]
fn opus_encodes_and_decodes_a_voice() {
    let input = voice(60);
    let mut enc = encoder(Codec::Voip);
    let mut dec = Opus::new();
    let mut out = Vec::new();
    let mut buf = [0u8; MAX_OPUS_PAYLOAD];
    for frame in input.chunks_exact(FRAME_SIZE) {
        let n = enc.encode(frame, FRAME_SIZE, &mut buf).unwrap();
        assert!(n > 2 && n <= MAX_OPUS_PAYLOAD);
        // SILK or hybrid (TOC configs 0 to 15), as libopus codes a voice in VOIP.
        assert!(buf[0] >> 3 < 16, "config {}", buf[0] >> 3);
        let pcm = dec.decode(&buf[..n]);
        assert_eq!(pcm.len(), FRAME_SIZE);
        out.extend(pcm);
    }
    let (c, lag) = correlation(&input, &out, 1200);
    assert!(c > 0.8, "correlation {c} at {lag}");
    let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
    assert!((rms(&out) / rms(&input) - 1.0).abs() < 0.3);
    // A lost frame is concealed (not silence right after a voice), recovered from FEC too.
    assert_eq!(dec.conceal().len(), FRAME_SIZE);
    let n = enc.encode(&input[..FRAME_SIZE], FRAME_SIZE, &mut buf).unwrap();
    assert_eq!(dec.recover(&buf[..n]).len(), FRAME_SIZE);
    // Garbage decodes to a silent frame, not a panic.
    assert_eq!(dec.decode(&[0xFF, 0x03, 0x01]).len(), FRAME_SIZE);
}

#[test]
fn voices_fade_with_distance_as_openal_linear_distance() {
    assert_eq!(distance_gain(48.0, 0.0), 1.0);
    assert_eq!(distance_gain(48.0, 24.0), 0.5);
    assert_eq!(distance_gain(48.0, 48.0), 0.0);
    assert_eq!(distance_gain(48.0, 100.0), 0.0);
    assert_eq!(distance_gain(0.0, 1.0), 0.0);
    // Beyond the distance (and a block), SVC drops the voice.
    let l = Listener {
        pos: [0.0, 64.0, 0.0],
        yaw: 0.0,
        pitch: 0.0,
    };
    assert!(positional(&l, [0.0, 64.0, 49.5], 48.0).is_none());
    assert_eq!(positional(&l, [0.0, 64.0, 48.5], 48.0), Some([0.0, 0.0]));
    let half = positional(&l, [0.0, 64.0, 24.0], 48.0).unwrap();
    let c = centre();
    assert!((half[0] - c[0] * 0.5).abs() < 1e-5 && (half[1] - c[1] * 0.5).abs() < 1e-5);
}

#[test]
fn voices_come_from_their_side() {
    // OpenAL Soft's stereo: 0.596 on each side in front, fully panned at 60 degrees.
    let c = centre();
    assert!((c[0] - 0.5957).abs() < 1e-3 && (c[0] - c[1]).abs() < 1e-6);
    let behind = stereo_gains(0.0, 0.0, -1.0);
    assert!((behind[0] - 0.4043).abs() < 1e-3);
    let right = stereo_gains(1.0, 0.0, 0.0);
    assert!(right[1] > 0.99 && right[0] < 0.01, "{right:?}");
    let sixty = stereo_gains(3f32.sqrt() / 2.0, 0.0, 0.5);
    assert!(sixty[0] < 0.01, "{sixty:?}");
    // Facing south (yaw 0): east (+x) is on the left; facing north, on the right.
    let south = Listener {
        pos: [0.0, 64.0, 0.0],
        yaw: 0.0,
        pitch: 0.0,
    };
    let g = positional(&south, [5.0, 64.0, 0.0], 48.0).unwrap();
    assert!(g[0] > g[1] * 5.0, "{g:?}");
    let north = Listener { yaw: 180.0, ..south };
    let g = positional(&north, [5.0, 64.0, 0.0], 48.0).unwrap();
    assert!(g[1] > g[0] * 5.0, "{g:?}");
    // Facing west (yaw 90): a voice ahead (-x) is centred.
    let west = Listener { yaw: 90.0, ..south };
    let g = positional(&west, [-5.0, 64.0, 0.0], 48.0).unwrap();
    assert!((g[0] - g[1]).abs() < 1e-4, "{g:?}");
    // Looking straight down, a voice ahead on the ground is still in front.
    let down = Listener { pitch: 90.0, ..south };
    let [r, _, f] = down.local([0.0, 64.0, 5.0]);
    assert!(r.abs() < 1e-4 && f.abs() < 1e-4);
}

#[derive(Debug, Clone)]
struct P(i64, bool);
impl Sequenced for P {
    fn seq(&self) -> i64 {
        self.0
    }
    fn is_end(&self) -> bool {
        self.1
    }
}

fn seqs(v: &[P]) -> Vec<i64> {
    v.iter().map(|p| p.0).collect()
}

#[test]
fn the_jitter_buffer_puts_packets_in_order() {
    let mut b = JitterBuffer::new(3);
    assert_eq!(seqs(&b.push(P(0, false))), [0]);
    assert_eq!(seqs(&b.push(P(2, false))), Vec::<i64>::new());
    assert_eq!(b.held(), 1);
    assert_eq!(seqs(&b.push(P(1, false))), [1, 2]);
    // A lost packet: the later ones wait until more than three wait.
    assert!(b.push(P(4, false)).is_empty());
    assert!(b.push(P(5, false)).is_empty());
    assert!(b.push(P(6, false)).is_empty());
    assert_eq!(seqs(&b.push(P(7, false))), [4, 5, 6, 7]);
    // An end out of order flushes what waits.
    assert!(b.push(P(9, false)).is_empty());
    assert_eq!(seqs(&b.push(P(10, true))), [9, 10]);
    b.clear();
    assert_eq!(seqs(&b.push(P(100, false))), [100]);
    // No threshold: as they come.
    let mut none = JitterBuffer::new(0);
    assert_eq!(seqs(&none.push(P(5, false))), [5]);
    assert_eq!(seqs(&none.push(P(3, false))), [3]);
}

/// A decoder that notes what it was asked.
#[derive(Default)]
struct Log(Vec<String>);
impl FrameDecoder for Log {
    fn decode(&mut self, data: &[u8]) -> Vec<f32> {
        self.0.push(format!("decode {}", data[0]));
        vec![0.5; FRAME_SIZE]
    }
    fn conceal(&mut self) -> Vec<f32> {
        self.0.push("conceal".into());
        vec![0.0; FRAME_SIZE]
    }
    fn recover(&mut self, next: &[u8]) -> Vec<f32> {
        self.0.push(format!("recover from {}", next[0]));
        vec![0.25; FRAME_SIZE]
    }
    fn reset(&mut self) {
        self.0.push("reset".into());
    }
}

fn packet(seq: i64) -> Packet<()> {
    Packet {
        seq,
        data: vec![seq as u8],
        meta: (),
    }
}

fn frames(played: &[Played<()>]) -> Vec<usize> {
    played
        .iter()
        .map(|p| match p {
            Played::Frames(f, _) => f.len(),
            Played::End(_) => 0,
        })
        .collect()
}

#[test]
fn lost_packets_are_concealed_then_recovered_in_svcs_order() {
    let mut s = Speaker::new(Log::default());
    assert_eq!(frames(&s.push(packet(0))), [1]);
    assert_eq!(frames(&s.push(packet(1))), [1]);
    // 2 and 3 are lost: 4..7 wait, then 4 plays as conceal (n - 1 = 1), recover, decode.
    for seq in 4..7 {
        assert!(s.push(packet(seq)).is_empty());
    }
    assert_eq!(frames(&s.push(packet(7))), [3, 1, 1, 1]);
    assert_eq!(
        s.decoder().0,
        [
            "decode 0",
            "decode 1",
            "conceal",
            "recover from 4",
            "decode 4",
            "decode 5",
            "decode 6",
            "decode 7"
        ]
    );
    assert_eq!(s.lost, 2);
    // An old packet is dropped.
    assert!(s.push(packet(3)).is_empty());
    // The end: the decoder starts over, and so does the order.
    let end = Packet {
        seq: 8,
        data: Vec::new(),
        meta: (),
    };
    assert!(matches!(s.push(end)[..], [Played::End(())]));
    assert_eq!(s.decoder().0.last().unwrap(), "reset");
    assert_eq!(frames(&s.push(packet(0))), [1]);
}

#[test]
fn a_long_gap_starts_over_instead_of_concealing() {
    let mut s = Speaker::new(Log::default());
    s.push(packet(0));
    let gap = OUTPUT_BUFFER_SIZE + 2;
    let mut out = Vec::new();
    for seq in gap..gap + 4 {
        out.extend(s.push(packet(seq)));
    }
    assert_eq!(frames(&out), [1, 1, 1, 1]);
    assert_eq!(
        s.decoder().0[..3],
        ["decode 0".to_owned(), "reset".into(), format!("decode {gap}")]
    );
    // Raw PCM (negative sequence) plays as it is.
    let raw = Packet {
        seq: -1,
        data: [0x00, 0x40, 0x00, 0xC0].to_vec(),
        meta: (),
    };
    let mut s = Speaker::new(Log::default());
    let Played::Frames(f, ()) = &s.push(raw)[0] else {
        panic!()
    };
    assert_eq!(f[0], [0.5, -0.5]);
}

#[test]
fn the_mixer_waits_for_two_frames_and_follows_the_gains() {
    let mut m = Mixer::new();
    let one = vec![0.5f32; FRAME_SIZE];
    let mut out = vec![0f32; FRAME_SIZE * 2];
    m.push(1, &one, [1.0, 0.0]);
    m.render(&mut out);
    assert!(out.iter().all(|&s| s == 0.0), "one frame waits");
    for _ in 1..PREBUFFER_FRAMES {
        m.push(1, &one, [1.0, 0.0]);
    }
    m.render(&mut out);
    assert!((out[0] - 0.5).abs() < 1e-6 && out[1] == 0.0);
    // A move to the right fades over the frame.
    m.push(1, &one, [0.0, 1.0]);
    m.render(&mut out);
    m.render(&mut out);
    let last = &out[out.len() - 2..];
    assert!(last[0].abs() < 1e-3 && (last[1] - 0.5).abs() < 1e-3, "{last:?}");
    // Two speakers add up; the volume scales them; the sum is clipped.
    let mut m = Mixer::new();
    m.volume = 2.0;
    for ch in [1, 2] {
        m.push(ch, &one, [1.0, 1.0]);
        m.push(ch, &one, [1.0, 1.0]);
    }
    m.render(&mut out);
    assert_eq!(out[0], 1.0);
    // An ended speaker's last frame plays without waiting; deafened, nothing.
    let mut m = Mixer::new();
    m.push(3, &one, [1.0, 1.0]);
    m.end(3);
    m.render(&mut out);
    assert!((out[0] - 0.5).abs() < 1e-6);
    assert_eq!(m.active(), 0);
    m.deafened = true;
    m.push(3, &one, [1.0, 1.0]);
    assert_eq!(m.queued(3), 0);
    // A speaker far behind loses their oldest audio.
    let mut m = Mixer::new();
    for _ in 0..40 {
        m.push(4, &one, [1.0, 1.0]);
    }
    assert!(m.queued(4) <= 12 * FRAME_SIZE);
}

#[test]
fn push_to_talk_and_voice_activation_decide_as_svc() {
    let loud = || vec![0.2f32; FRAME_SIZE];
    let quiet = || vec![0.0001f32; FRAME_SIZE];
    let mut p = MicProcessor::new();
    // Push to talk: while held, then 5 frames more.
    assert!(!p.process(&mut loud(), Activation::PushToTalk, 0.0, -50.0, false, false));
    assert!(p.process(&mut loud(), Activation::PushToTalk, 0.0, -50.0, true, false));
    let after: Vec<bool> = (0..PTT_DELAY_FRAMES + 1)
        .map(|_| p.process(&mut loud(), Activation::PushToTalk, 0.0, -50.0, false, false))
        .collect();
    assert_eq!(after.iter().filter(|&&b| b).count(), PTT_DELAY_FRAMES as usize);
    assert!(!after.last().unwrap());
    // Muted: never.
    assert!(!p.process(&mut loud(), Activation::PushToTalk, 0.0, -50.0, true, true));
    // Voice activation: above the threshold (-50 dB), then 25 frames more.
    let mut p = MicProcessor::new();
    assert!(!p.process(&mut quiet(), Activation::Voice, 0.0, -50.0, false, false));
    assert!(p.process(&mut loud(), Activation::Voice, 0.0, -50.0, false, false));
    let after = (0..VOICE_DELAY_FRAMES + 1)
        .filter(|_| p.process(&mut quiet(), Activation::Voice, 0.0, -50.0, false, false))
        .count();
    assert_eq!(after, VOICE_DELAY_FRAMES as usize);
    // The gain (dB) lifts the frame, and the limiter keeps it from clipping.
    let mut p = MicProcessor::new();
    let mut f = vec![0.1f32; FRAME_SIZE];
    p.process(&mut f, Activation::Voice, 6.0, -50.0, false, false);
    assert!((f[0] - 0.1995).abs() < 1e-3, "{}", f[0]);
    let mut f = vec![0.5f32; FRAME_SIZE];
    p.process(&mut f, Activation::Voice, 24.0, -50.0, false, false);
    assert!(f[0] <= 0.99);
    assert!((peak_db(&[0.5]) + 6.02).abs() < 0.01);
    assert_eq!(peak_db(&[0.0]), -127.0);
    // The buffer hands out whole frames.
    let b = MicBuffer::new();
    b.push(&vec![0.1; FRAME_SIZE + 10]);
    assert_eq!(b.frame().unwrap().len(), FRAME_SIZE);
    assert!(b.frame().is_none());
    assert_eq!(b.len(), 10);
}

#[test]
fn resampling_keeps_the_pitch() {
    let mut r = Resampler::new(44_100, 48_000);
    let input: Vec<f32> = (0..44_100)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin())
        .collect();
    let mut out = Vec::new();
    for block in input.chunks(441) {
        r.process(block, &mut out);
    }
    assert!((out.len() as i64 - 48_000).abs() < 4, "{}", out.len());
    // Zero crossings: 880 a second either way.
    let crossings = |v: &[f32]| v.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    assert!((crossings(&out) as i64 - crossings(&input) as i64).abs() <= 1);
    let mut down = Resampler::new(48_000, 16_000);
    let mut o = Vec::new();
    down.process(&[0.0; 4800], &mut o);
    assert!((o.len() as i64 - 1600).abs() <= 1);
    let mut same = Resampler::new(48_000, 48_000);
    let mut o = Vec::new();
    same.process(&[0.25, 0.5], &mut o);
    assert_eq!(o, [0.25, 0.5]);
    let mut mono = Vec::new();
    to_mono(&[1.0, 0.0, 0.5, 0.5], 2, &mut mono);
    assert_eq!(mono, [0.5, 0.5]);
}

#[test]
fn settings_are_kept_as_text_within_their_ranges() {
    let s = Settings {
        activation: Activation::Voice,
        volume: 1.5,
        gain_db: -6.0,
        threshold_db: -40.0,
        muted: true,
        deafened: false,
    };
    assert_eq!(Settings::from_text(&s.to_text()), s);
    let wild = Settings::from_text("volume=9\ngain_db=-100\nthreshold_db=nan\nactivation=bogus\nmuted=yes\n");
    assert_eq!(wild.volume, 3.0);
    assert_eq!(wild.gain_db, -40.0);
    assert_eq!(wild.threshold_db, -50.0);
    assert_eq!(wild.activation, Activation::PushToTalk);
    assert!(!wild.muted);
    assert_eq!(Settings::from_text(""), Settings::default());
}
