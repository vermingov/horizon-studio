//! Feature-gated encode/decode **stage profiler** — the instrument for perf work.
//!
//! Zero cost unless the `profile` cargo feature is enabled: with it off,
//! [`scope`] is a no-op returning a ZST guard the optimizer elides entirely, so
//! release builds are byte-identical and the hot path is untouched. With it on,
//! each stage times itself into an atomic tick bucket; [`dump`] prints the
//! per-stage breakdown and [`snapshot`] returns a calibrated reading so a driver
//! can run many passes and take per-stage medians.
//!
//! Design mirrors `rusty_h264-common/src/prof.rs` (rdtsc ticks + wall-clock
//! anchor calibration). A [`Stage::Total`] scope wraps `OpusEncoder::encode()`;
//! the **`mgmt/other`** line is the residue (`Total − Σ stages`) — decompose it
//! until every line is named, or prove it equals the timer overhead
//! (`Σ calls × ~2 × tick cost`).

/// A timed pipeline stage. Order matters: everything before [`Total`](Stage::Total)
/// is a sub-component summed for the `mgmt/other` residue.
#[derive(Clone, Copy)]
pub enum Stage {
    // --- top-level (lib.rs encode()) ---
    /// hp_cutoff / f32→i16 conversion + SILK input resampling (down2 / down2_3).
    Resample = 0,
    // --- SILK encoder ---
    /// silk_vad_get_sa_q8 (voice activity detection).
    SilkVad = 1,
    /// silk_find_pitch_lags_fix (open-loop pitch analysis).
    SilkPitch = 2,
    /// silk_noise_shape_analysis_fix (shaping filter derivation).
    SilkNoise = 3,
    /// silk_find_pred_coefs_fix (LPC/LTP analysis + NLSF quantization).
    SilkPred = 4,
    /// silk_nsq / silk_nsq_del_dec (noise-shaping quantizer, incl. rate-loop reruns).
    SilkNsq = 5,
    /// silk_encode_indices + silk_encode_pulses (range coding of SILK symbols).
    SilkCode = 6,
    // --- CELT encoder ---
    /// Pre-emphasis + input/overlap buffer plumbing.
    CeltPreemph = 7,
    /// transient_analysis (short/long block decision).
    CeltTransient = 8,
    /// run_prefilter (pitch pre-filter incl. its pitch search).
    CeltPrefilter = 9,
    /// mode.mdct.forward calls (the forward MDCT(s)).
    CeltMdct = 10,
    /// compute_band_energies + normalise_bands.
    CeltBands = 11,
    /// quant_coarse_energy (coarse energy quantization + laplace coding).
    CeltCoarse = 12,
    /// tf_analysis + tf_encode (time-frequency resolution switching).
    CeltTf = 13,
    /// dynalloc_analysis + alloc_trim_analysis + clt_compute_allocation.
    CeltAlloc = 14,
    /// quant_fine_energy.
    CeltFine = 15,
    /// quant_all_bands (PVQ search + encode — the expected workhorse).
    CeltPvq = 16,
    /// Encoder-side synthesis after coding (denormalise/IMDCT for prefilter memory).
    CeltSynth = 17,
    // --- info-tier diagnostic scopes (nested inside SilkNsq; EXCLUDED from the
    //     residue sum via INFO_FIRST). Remove call sites after reading — at n_states
    //     × length calls their own rdtsc overhead inflates the enclosing stage. ---
    /// silk_noise_shape_quantizer_short_prediction (16-tap LPC dot product).
    SilkNsqLpc = 18,
    /// Warped shaping AR filter (serial recurrence) + RD decision, per state.
    SilkNsqShape = 19,
    /// Wraps the whole `OpusEncoder::encode()` call — the denominator.
    Total = 20,
}

/// Number of buckets.
pub const N: usize = 21;

/// Index of the first info-tier stage — buckets `INFO_FIRST..Total` are nested
/// diagnostics excluded from the `mgmt/other` residue sum.
pub const INFO_FIRST: usize = Stage::SilkNsqLpc as usize;

// Horizon Studio: the `profile` feature's timers are removed; only the no-op stays.
mod imp {
    use super::{N, Stage};

    /// No-op guard (ZST) — elided in release.
    pub struct Guard;

    #[inline(always)]
    pub fn scope(_s: Stage) -> Guard {
        Guard
    }
    #[inline(always)]
    pub fn reset() {}
    #[inline(always)]
    pub fn dump() {}
    #[inline(always)]
    pub fn snapshot() -> [(f64, u64); N] {
        [(0.0, 0); N]
    }
    #[inline(always)]
    pub fn name(_i: usize) -> &'static str {
        ""
    }
}

pub use imp::{Guard, dump, name, reset, scope, snapshot};
