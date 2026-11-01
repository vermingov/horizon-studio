#!/usr/bin/env python3
"""The voice chat's Opus (rusty-opus 1.0.2, vendored in mixins/horizon_studio/voice) against the system's libopus,
the codec every Simple Voice Chat client uses (natively, or its Java port of it):

- packets our encoder makes (48 kHz mono, VOIP, 20 ms frames, as the voice chat sends) decode in libopus to the
  sound that went in;
- libopus's VOIP packets (SILK and hybrid, at its automatic bitrate and at lower ones) decode in ours as libopus
  decodes them, and so do lost packets: concealed, and recovered from the next packet's forward error correction.

    python3 tools/opus_interop.py           # needs libopus (libopus.so.0 / libopus.dylib) and numpy

It builds tools/opus_interop (a small command around the voice crate) and drives it with files.
"""
import ctypes
import ctypes.util
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
RATE, FRAME, VOIP = 48000, 960, 2048
SET_BITRATE, SET_INBAND_FEC, SET_PACKET_LOSS_PERC = 4002, 4012, 4014


def libopus():
    for name in [ctypes.util.find_library("opus"), "libopus.so.0", "/lib/x86_64-linux-gnu/libopus.so.0", "libopus.dylib"]:
        if name:
            try:
                return ctypes.CDLL(name)
            except OSError:
                pass
    sys.exit("libopus not found")


LIB = libopus()
LIB.opus_get_version_string.restype = ctypes.c_char_p


def build():
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "tools" / "opus_interop" / "target"))
    subprocess.run(["cargo", "build", "-q", "--manifest-path", ROOT / "tools" / "opus_interop" / "Cargo.toml"],
                   check=True, env={**os.environ, "CARGO_TARGET_DIR": str(target)})
    return target / "debug" / "opus-interop"


def speech(seconds=6):
    """A voiced sound like speech: a glottal pulse train with a wandering pitch through moving formants, in
    syllables, with pauses and a little breath noise."""
    rng = np.random.default_rng(1)
    n = RATE * seconds
    t = np.arange(n) / RATE
    f0 = 120 + 40 * np.sin(2 * np.pi * 0.7 * t) + 15 * np.sin(2 * np.pi * 3.1 * t)
    src = (np.diff(np.floor(np.cumsum(f0 / RATE)), prepend=0) > 0).astype(float)
    src = np.convolve(src, np.exp(-np.arange(200) / 30.0) * np.sin(np.arange(200) / 8.0))[:n]
    out = np.zeros(n)
    for f, bw, g in ((None, 80, 1.0), (None, 120, 0.5), (2600, 200, 0.25)):
        r = np.exp(-np.pi * bw / RATE)
        y1 = y2 = 0.0
        for i in range(n):
            fi = f if f else (500 + 250 * np.sin(2 * np.pi * 1.3 * t[i]) if bw == 80 else 1500 + 700 * np.sin(2 * np.pi * 0.9 * t[i] + 1))
            v = src[i] + 2 * r * np.cos(2 * np.pi * fi / RATE) * y1 - r * r * y2
            y2, y1 = y1, v
            out[i] += g * v
    env = np.clip(np.sin(2 * np.pi * 2.2 * t), 0, None) ** 0.5 * (np.sin(2 * np.pi * 0.25 * t) > -0.6)
    sig = out * env + rng.normal(0, 1, n) * (np.sin(2 * np.pi * 2.2 * t + 2.5) > 0.9) * 0.05
    sig = sig / np.max(np.abs(sig)) * 0.5
    return sig[: len(sig) // FRAME * FRAME].astype(np.float32)


def write_pkts(path, pkts):
    with open(path, "wb") as f:
        for p in pkts:
            f.write(struct.pack("<H", len(p)) + p)


def read_pkts(path):
    data, pkts, i = Path(path).read_bytes(), [], 0
    while i < len(data):
        n = struct.unpack_from("<H", data, i)[0]
        pkts.append(data[i + 2:i + 2 + n])
        i += 2 + n
    return pkts


def lib_encode(sig, bitrate=None):
    err = ctypes.c_int()
    enc = LIB.opus_encoder_create(RATE, 1, VOIP, ctypes.byref(err))
    LIB.opus_encoder_ctl(enc, SET_INBAND_FEC, ctypes.c_int(1))
    LIB.opus_encoder_ctl(enc, SET_PACKET_LOSS_PERC, ctypes.c_int(5))
    if bitrate:
        LIB.opus_encoder_ctl(enc, SET_BITRATE, ctypes.c_int(bitrate))
    buf, pkts = (ctypes.c_ubyte * 1275)(), []
    for i in range(0, len(sig), FRAME):
        n = LIB.opus_encode_float(enc, (ctypes.c_float * FRAME)(*sig[i:i + FRAME]), FRAME, buf, 1275)
        assert n > 0, n
        pkts.append(bytes(buf[:n]))
    LIB.opus_encoder_destroy(enc)
    return pkts


def lib_decode(pkts, loss=None):
    """libopus's decode; with `loss` "plc" or "fec", every 7th packet lost as opus-interop loses it."""
    err = ctypes.c_int()
    dec = LIB.opus_decoder_create(RATE, 1, ctypes.byref(err))
    pcm, out = (ctypes.c_float * FRAME)(), []
    for k, p in enumerate(pkts):
        b = (ctypes.c_ubyte * len(p)).from_buffer_copy(p)
        if loss and k % 7 == 3:
            if loss == "plc":
                LIB.opus_decode_float(dec, None, 0, pcm, FRAME, 0)
                out.append(np.frombuffer(bytes(pcm), dtype=np.float32).copy())
            continue
        if loss == "fec" and k % 7 == 4:
            LIB.opus_decode_float(dec, b, len(p), pcm, FRAME, 1)
            out.append(np.frombuffer(bytes(pcm), dtype=np.float32).copy())
        assert LIB.opus_decode_float(dec, b, len(p), pcm, FRAME, 0) == FRAME
        out.append(np.frombuffer(bytes(pcm), dtype=np.float32).copy())
    LIB.opus_decoder_destroy(dec)
    return np.concatenate(out)


def corr(ref, got, maxlag=1500):
    """The best correlation of `got` with `ref` delayed by up to `maxlag` (the codec's delay)."""
    n = min(len(ref), len(got)) - maxlag
    return max(np.corrcoef(ref[:n], got[lag:lag + n])[0, 1] for lag in range(maxlag))


def snr(ref, got):
    n = min(len(ref), len(got))
    r, g = ref[:n].astype(np.float64), got[:n].astype(np.float64)
    return 10 * np.log10(np.sum(r * r) / max(np.sum((r - g) ** 2), 1e-30))


def modes(pkts):
    names = {}
    for p in pkts:
        c = p[0] >> 3
        m = "SILK" if c < 12 else "hybrid" if c < 16 else "CELT"
        names[m] = names.get(m, 0) + 1
    return names


def main():
    tool = build()
    print("libopus", LIB.opus_get_version_string().decode())
    sig = speech()
    ok = True
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        sig.tofile(d / "in.f32")

        def ours(mode, inp, out, *extra):
            subprocess.run([tool, mode, inp, out, *extra], check=True)
            return np.fromfile(out, dtype=np.float32) if mode != "enc" else read_pkts(out)

        for bitrate in (None, "24000", "12000"):
            pkts = ours("enc", d / "in.f32", d / "ours.pkts", *([bitrate] if bitrate else []))
            c = corr(sig, lib_decode(pkts))
            print(f"ours -> libopus ({bitrate or 'auto'} bps, {modes(pkts)}): correlation with the input {c:.3f}")
            ok &= c > 0.85
        for bitrate in (None, 24000, 12000):
            pkts = lib_encode(sig, bitrate)
            write_pkts(d / "lib.pkts", pkts)
            for loss in (None, "plc", "fec"):
                got = ours(loss or "dec", d / "lib.pkts", d / "out.f32")
                ref = lib_decode(pkts, loss)
                s = snr(ref, got)
                print(f"libopus ({bitrate or 'auto'} bps, {modes(pkts)}) -> ours, {loss or 'no loss'}: "
                      f"{s:.0f} dB from libopus's own decode, correlation with the input {corr(sig, got):.3f}")
                ok &= len(got) == len(ref) and s > 40
    print("PASS" if ok else "FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
