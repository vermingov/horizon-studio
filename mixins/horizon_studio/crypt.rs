// Cryptography and compression for script mods, under the executor-style
// `crypt` library and `lz4compress`/`lz4decompress`: hashes (MD5, SHA-1,
// SHA-2 and SHA-3), HMAC, base64, AES (ECB, CBC, CFB, OFB, CTR and GCM),
// random bytes, and LZ4's block format. SHA-1 and SHA-2 come from the sha1
// and sha2 crates, which the game has; everything else is written here, and
// tests/crypt.rs checks it against the standards' test vectors. Errors are
// plain English, for Lua to show. AES and GHASH use lookup tables: fast, but
// not hardened against an attacker who can time the cache on the same machine.

use sha2::Digest;
use std::cell::RefCell;
use std::hash::{BuildHasher, RandomState};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------- hex

/// Bytes as lowercase hex ("00ff").
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(char::from(DIGITS[usize::from(b >> 4)]));
        s.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    s
}

/// Hex in either case as bytes; spaces and line breaks between digits are skipped.
pub fn from_hex(text: impl AsRef<[u8]>) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.as_ref().len() / 2);
    let mut high = None;
    for &c in text.as_ref() {
        let v = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            c if c.is_ascii_whitespace() => continue,
            _ => return Err(format!("not hex: it has {}", describe(c))),
        };
        match high.take() {
            None => high = Some(v),
            Some(h) => out.push((h << 4) | v),
        }
    }
    if high.is_some() {
        return Err("not hex: it has an odd number of digits".into());
    }
    Ok(out)
}

/// A byte, for an error message: 'x', or its value when it isn't printable.
fn describe(c: u8) -> String {
    if c.is_ascii_graphic() {
        format!("'{}'", char::from(c))
    } else {
        format!("byte {c:#04x}")
    }
}

/// The bytes as an array (`bytes` is exactly N long).
fn array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut a = [0; N];
    a.copy_from_slice(bytes);
    a
}

// ---------------------------------------------------------------- hashes

/// A hash function, as `crypt.hash` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hash {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_224,
    Sha3_256,
    Sha3_384,
    Sha3_512,
}

impl Hash {
    pub const ALL: [Hash; 10] = [
        Hash::Md5,
        Hash::Sha1,
        Hash::Sha224,
        Hash::Sha256,
        Hash::Sha384,
        Hash::Sha512,
        Hash::Sha3_224,
        Hash::Sha3_256,
        Hash::Sha3_384,
        Hash::Sha3_512,
    ];

    /// By name, in any case and with or without dashes and underscores:
    /// "md5", "sha1", "sha256", "SHA-512", "sha3-256", "sha3_384".
    pub fn from_name(name: &str) -> Option<Hash> {
        let want: String = name
            .trim()
            .chars()
            .filter(|c| !matches!(c, '-' | '_'))
            .map(|c| c.to_ascii_lowercase())
            .collect();
        Hash::ALL.into_iter().find(|h| h.name().replace('-', "") == want)
    }

    /// Its usual name: "md5", "sha1", "sha256", "sha3-256".
    pub fn name(self) -> &'static str {
        match self {
            Hash::Md5 => "md5",
            Hash::Sha1 => "sha1",
            Hash::Sha224 => "sha224",
            Hash::Sha256 => "sha256",
            Hash::Sha384 => "sha384",
            Hash::Sha512 => "sha512",
            Hash::Sha3_224 => "sha3-224",
            Hash::Sha3_256 => "sha3-256",
            Hash::Sha3_384 => "sha3-384",
            Hash::Sha3_512 => "sha3-512",
        }
    }

    /// The digest's length in bytes.
    pub fn size(self) -> usize {
        match self {
            Hash::Md5 => 16,
            Hash::Sha1 => 20,
            Hash::Sha224 | Hash::Sha3_224 => 28,
            Hash::Sha256 | Hash::Sha3_256 => 32,
            Hash::Sha384 | Hash::Sha3_384 => 48,
            Hash::Sha512 | Hash::Sha3_512 => 64,
        }
    }

    /// The block HMAC pads its key to, in bytes (for SHA-3, the sponge's rate).
    pub fn block_size(self) -> usize {
        match self {
            Hash::Md5 | Hash::Sha1 | Hash::Sha224 | Hash::Sha256 => 64,
            Hash::Sha384 | Hash::Sha512 => 128,
            Hash::Sha3_224 | Hash::Sha3_256 | Hash::Sha3_384 | Hash::Sha3_512 => 200 - 2 * self.size(),
        }
    }

    /// The digest of `data`.
    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        let mut h = Hasher::new(self);
        h.update(data);
        h.finish()
    }

    /// The digest of `data`, in lowercase hex.
    pub fn hex(self, data: &[u8]) -> String {
        hex(&self.digest(data))
    }
}

/// MD5 (RFC 1321). Broken for signatures; still what many formats checksum with.
pub fn md5(data: &[u8]) -> [u8; 16] {
    let mut h = Md5::new();
    h.update(data);
    h.finish()
}

/// SHA-1 (FIPS 180-4), from the sha1 crate.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    array(&sha1::Sha1::digest(data))
}

/// SHA-224 (FIPS 180-4), from the sha2 crate.
pub fn sha224(data: &[u8]) -> [u8; 28] {
    array(&sha2::Sha224::digest(data))
}

/// SHA-256 (FIPS 180-4), from the sha2 crate.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    array(&sha2::Sha256::digest(data))
}

/// SHA-384 (FIPS 180-4), from the sha2 crate.
pub fn sha384(data: &[u8]) -> [u8; 48] {
    array(&sha2::Sha384::digest(data))
}

/// SHA-512 (FIPS 180-4), from the sha2 crate.
pub fn sha512(data: &[u8]) -> [u8; 64] {
    array(&sha2::Sha512::digest(data))
}

/// SHA3-224 (FIPS 202).
pub fn sha3_224(data: &[u8]) -> [u8; 28] {
    array(&Hash::Sha3_224.digest(data))
}

/// SHA3-256 (FIPS 202).
pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    array(&Hash::Sha3_256.digest(data))
}

/// SHA3-384 (FIPS 202).
pub fn sha3_384(data: &[u8]) -> [u8; 48] {
    array(&Hash::Sha3_384.digest(data))
}

/// SHA3-512 (FIPS 202).
pub fn sha3_512(data: &[u8]) -> [u8; 64] {
    array(&Hash::Sha3_512.digest(data))
}

/// A hash fed piece by piece, for data that comes in parts:
/// `new`, any number of `update`s, then `finish`.
#[derive(Clone)]
pub struct Hasher(State);

#[derive(Clone)]
enum State {
    Md5(Md5),
    Sha1(sha1::Sha1),
    Sha224(sha2::Sha224),
    Sha256(sha2::Sha256),
    Sha384(sha2::Sha384),
    Sha512(sha2::Sha512),
    Sha3(Keccak),
}

impl Hasher {
    pub fn new(hash: Hash) -> Hasher {
        Hasher(match hash {
            Hash::Md5 => State::Md5(Md5::new()),
            Hash::Sha1 => State::Sha1(sha1::Sha1::new()),
            Hash::Sha224 => State::Sha224(sha2::Sha224::new()),
            Hash::Sha256 => State::Sha256(sha2::Sha256::new()),
            Hash::Sha384 => State::Sha384(sha2::Sha384::new()),
            Hash::Sha512 => State::Sha512(sha2::Sha512::new()),
            Hash::Sha3_224 | Hash::Sha3_256 | Hash::Sha3_384 | Hash::Sha3_512 => State::Sha3(Keccak::new(hash.size())),
        })
    }

    pub fn update(&mut self, data: &[u8]) {
        match &mut self.0 {
            State::Md5(h) => h.update(data),
            State::Sha1(h) => h.update(data),
            State::Sha224(h) => h.update(data),
            State::Sha256(h) => h.update(data),
            State::Sha384(h) => h.update(data),
            State::Sha512(h) => h.update(data),
            State::Sha3(h) => h.update(data),
        }
    }

    /// The digest of everything given to `update`.
    pub fn finish(self) -> Vec<u8> {
        match self.0 {
            State::Md5(h) => h.finish().to_vec(),
            State::Sha1(h) => h.finalize().to_vec(),
            State::Sha224(h) => h.finalize().to_vec(),
            State::Sha256(h) => h.finalize().to_vec(),
            State::Sha384(h) => h.finalize().to_vec(),
            State::Sha512(h) => h.finalize().to_vec(),
            State::Sha3(h) => h.finish(),
        }
    }
}

/// HMAC (RFC 2104) of `data` under `key`, with any of the hashes.
pub fn hmac(hash: Hash, key: &[u8], data: &[u8]) -> Vec<u8> {
    let block = hash.block_size();
    let mut k = if key.len() > block {
        hash.digest(key)
    } else {
        key.to_vec()
    };
    k.resize(block, 0);
    let pad = |with: u8| k.iter().map(|b| b ^ with).collect::<Vec<u8>>();
    let mut inner = Hasher::new(hash);
    inner.update(&pad(0x36));
    inner.update(data);
    let inner = inner.finish();
    let mut outer = Hasher::new(hash);
    outer.update(&pad(0x5c));
    outer.update(&inner);
    outer.finish()
}

/// HMAC of `data` under `key`, in lowercase hex.
pub fn hmac_hex(hash: Hash, key: &[u8], data: &[u8]) -> String {
    hex(&hmac(hash, key, data))
}

// MD5's per-step shifts (four per round) and constants, floor(abs(sin(i + 1)) * 2^32).
const MD5_S: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
const MD5_K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

#[derive(Clone)]
struct Md5 {
    state: [u32; 4],
    buf: [u8; 64],
    used: usize,
    len: u64,
}

impl Md5 {
    fn new() -> Md5 {
        Md5 {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            buf: [0; 64],
            used: 0,
            len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.len = self.len.wrapping_add(data.len() as u64);
        if self.used > 0 {
            let take = (64 - self.used).min(data.len());
            self.buf[self.used..self.used + take].copy_from_slice(&data[..take]);
            self.used += take;
            data = &data[take..];
            if self.used < 64 {
                return;
            }
            let block = self.buf;
            md5_block(&mut self.state, &block);
            self.used = 0;
        }
        let (blocks, rest) = data.as_chunks::<64>();
        for block in blocks {
            md5_block(&mut self.state, block);
        }
        self.buf[..rest.len()].copy_from_slice(rest);
        self.used = rest.len();
    }

    fn finish(mut self) -> [u8; 16] {
        let bits = self.len.wrapping_mul(8);
        // A 1 bit, zeros up to 8 bytes short of a block, then the length in bits.
        let pad = if self.used < 56 {
            56 - self.used
        } else {
            120 - self.used
        };
        let mut tail = [0u8; 64];
        tail[0] = 0x80;
        self.update(&tail[..pad]);
        self.update(&bits.to_le_bytes());
        let mut out = [0u8; 16];
        for (o, w) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *o = w.to_le_bytes();
        }
        out
    }
}

fn md5_block(state: &mut [u32; 4], block: &[u8; 64]) {
    let words = block.as_chunks::<4>().0;
    let m: [u32; 16] = std::array::from_fn(|i| u32::from_le_bytes(words[i]));
    let [mut a, mut b, mut c, mut d] = *state;
    for (i, &k) in MD5_K.iter().enumerate() {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let f = f.wrapping_add(a).wrapping_add(k).wrapping_add(m[g]);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(f.rotate_left(MD5_S[i / 16 * 4 + i % 4]));
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d]) {
        *s = s.wrapping_add(v);
    }
}

// Keccak-f[1600]'s round constants, and the rho rotations and pi lane order
// as one walk through the lanes (lane x + 5y).
const KECCAK_RC: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];
const KECCAK_RHO: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];
const KECCAK_PI: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

fn keccak_f(a: &mut [u64; 25]) {
    for rc in KECCAK_RC {
        // theta
        let c: [u64; 5] = std::array::from_fn(|x| a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20]);
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                a[5 * y + x] ^= d;
            }
        }
        // rho and pi
        let mut last = a[1];
        for (&j, &r) in KECCAK_PI.iter().zip(&KECCAK_RHO) {
            let next = a[j];
            a[j] = last.rotate_left(r);
            last = next;
        }
        // chi
        for row in a.as_chunks_mut::<5>().0 {
            let r = *row;
            for x in 0..5 {
                row[x] = r[x] ^ (!r[(x + 1) % 5] & r[(x + 2) % 5]);
            }
        }
        // iota
        a[0] ^= rc;
    }
}

/// SHA-3's sponge: bytes are XORed straight into the state's lanes.
#[derive(Clone)]
struct Keccak {
    lanes: [u64; 25],
    /// Bytes absorbed per permutation, and how many of this block are in.
    rate: usize,
    pos: usize,
    out: usize,
}

impl Keccak {
    fn new(out: usize) -> Keccak {
        Keccak {
            lanes: [0; 25],
            rate: 200 - 2 * out,
            pos: 0,
            out,
        }
    }

    fn xor_byte(&mut self, i: usize, b: u8) {
        self.lanes[i / 8] ^= u64::from(b) << (8 * (i % 8));
    }

    fn update(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            if self.pos == 0 && data.len() >= self.rate {
                let (block, rest) = data.split_at(self.rate);
                for (lane, w) in self.lanes.iter_mut().zip(block.as_chunks::<8>().0) {
                    *lane ^= u64::from_le_bytes(*w);
                }
                keccak_f(&mut self.lanes);
                data = rest;
                continue;
            }
            let take = (self.rate - self.pos).min(data.len());
            for &b in &data[..take] {
                self.xor_byte(self.pos, b);
                self.pos += 1;
            }
            data = &data[take..];
            if self.pos == self.rate {
                keccak_f(&mut self.lanes);
                self.pos = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        // SHA-3's domain bits (01), then the pad10*1 padding.
        self.xor_byte(self.pos, 0x06);
        self.xor_byte(self.rate - 1, 0x80);
        keccak_f(&mut self.lanes);
        self.lanes.iter().flat_map(|l| l.to_le_bytes()).take(self.out).collect()
    }
}

// ---------------------------------------------------------------- base64

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 (RFC 4648): the standard alphabet, padded with '='.
pub fn base64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut push = |n: u32, chars: usize| {
        for shift in [18, 12, 6, 0].into_iter().take(chars) {
            s.push(char::from(BASE64[((n >> shift) & 63) as usize]));
        }
    };
    let (groups, rest) = data.as_chunks::<3>();
    for &[a, b, c] in groups {
        push((u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c), 4);
    }
    match *rest {
        [a] => {
            push(u32::from(a) << 16, 2);
            s.push_str("==");
        }
        [a, b] => {
            push((u32::from(a) << 16) | (u32::from(b) << 8), 3);
            s.push('=');
        }
        _ => {}
    }
    s
}

/// Base64 in the standard alphabet as bytes. The '=' padding may be left
/// off and whitespace (line breaks too) is skipped; any other character
/// outside the alphabet is an error.
pub fn base64_decode(text: impl AsRef<[u8]>) -> Result<Vec<u8>, String> {
    let text = text.as_ref();
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 2);
    let (mut acc, mut bits, mut chars, mut pad) = (0u32, 0u32, 0usize, 0usize);
    for &c in text {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                pad += 1;
                continue;
            }
            b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => continue,
            _ => return Err(format!("not base64: it has {}", describe(c))),
        };
        if pad > 0 {
            return Err("not base64: it goes on after its '=' padding".into());
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        chars += 1;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if chars % 4 == 1 {
        return Err("not base64: its length is one character off".into());
    }
    if pad > 0 && (pad > 2 || !(chars + pad).is_multiple_of(4)) {
        return Err("not base64: its '=' padding is the wrong length".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- AES

const fn xtime(x: u8) -> u8 {
    (x << 1) ^ ((x >> 7) * 0x1b)
}

/// Multiplication in AES's field, GF(2^8) modulo x^8 + x^4 + x^3 + x + 1.
const fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

/// The S-box, from its definition: the field inverse (x^254), then the affine map.
const fn make_sbox() -> [u8; 256] {
    let mut s = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let (mut inv, mut base, mut e) = (1u8, i as u8, 254u32);
        while e > 0 {
            if e & 1 == 1 {
                inv = gmul(inv, base);
            }
            base = gmul(base, base);
            e >>= 1;
        }
        s[i] = inv ^ inv.rotate_left(1) ^ inv.rotate_left(2) ^ inv.rotate_left(3) ^ inv.rotate_left(4) ^ 0x63;
        i += 1;
    }
    s
}

const fn invert(s: &[u8; 256]) -> [u8; 256] {
    let mut inv = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        inv[s[i] as usize] = i as u8;
        i += 1;
    }
    inv
}

/// A round's four tables: a byte through the S-box `s`, times the first
/// column of the (Inv)MixColumns matrix (`mix`); each next table is the
/// one before rotated a byte, for the state's next row.
const fn make_tables(s: &[u8; 256], mix: [u8; 4]) -> [[u32; 256]; 4] {
    let mut t = [[0u32; 256]; 4];
    let mut i = 0;
    while i < 256 {
        let v = s[i];
        let w = u32::from_be_bytes([gmul(v, mix[0]), gmul(v, mix[1]), gmul(v, mix[2]), gmul(v, mix[3])]);
        t[0][i] = w;
        t[1][i] = w.rotate_right(8);
        t[2][i] = w.rotate_right(16);
        t[3][i] = w.rotate_right(24);
        i += 1;
    }
    t
}

const S: [u8; 256] = make_sbox();
static SBOX: [u8; 256] = S;
static INV_SBOX: [u8; 256] = invert(&S);
static TE: [[u32; 256]; 4] = make_tables(&S, [2, 1, 1, 3]);
static TD: [[u32; 256]; 4] = make_tables(&invert(&S), [14, 9, 13, 11]);

/// One column of a full round: byte 0 of `w0`, 1 of `w1`, 2 of `w2` and 3 of
/// `w3` (ShiftRows' choice) through the tables, then the round key.
fn column(t: &[[u32; 256]; 4], w0: [u8; 4], w1: [u8; 4], w2: [u8; 4], w3: [u8; 4], key: u32) -> u32 {
    t[0][usize::from(w0[0])] ^ t[1][usize::from(w1[1])] ^ t[2][usize::from(w2[2])] ^ t[3][usize::from(w3[3])] ^ key
}

/// One column of the last round, which has no MixColumns.
fn last_column(s: &[u8; 256], w0: [u8; 4], w1: [u8; 4], w2: [u8; 4], w3: [u8; 4], key: u32) -> u32 {
    let b = |w: [u8; 4], i: usize| s[usize::from(w[i])];
    u32::from_be_bytes([b(w0, 0), b(w1, 1), b(w2, 2), b(w3, 3)]) ^ key
}

fn sub_word(w: u32) -> u32 {
    u32::from_be_bytes(w.to_be_bytes().map(|b| SBOX[usize::from(b)]))
}

/// AES (FIPS 197) with a 128-, 192- or 256-bit key: its round keys, for
/// encrypting and decrypting 16-byte blocks.
#[derive(Clone)]
pub struct Aes {
    enc: [u32; 60],
    /// The equivalent inverse cipher's: in reverse, InvMixColumns applied.
    dec: [u32; 60],
    rounds: usize,
}

impl Aes {
    /// The key's length picks AES-128, AES-192 or AES-256.
    pub fn new(key: &[u8]) -> Result<Aes, String> {
        let nk = match key.len() {
            16 | 24 | 32 => key.len() / 4,
            n => return Err(format!("an AES key is 16, 24 or 32 bytes (this one is {n})")),
        };
        let rounds = nk + 6;
        let words = 4 * (rounds + 1);
        let mut enc = [0u32; 60];
        for (w, k) in enc.iter_mut().zip(key.as_chunks::<4>().0) {
            *w = u32::from_be_bytes(*k);
        }
        let mut rcon = 1u8;
        for i in nk..words {
            let mut t = enc[i - 1];
            if i % nk == 0 {
                t = sub_word(t.rotate_left(8)) ^ (u32::from(rcon) << 24);
                rcon = xtime(rcon);
            } else if nk > 6 && i % nk == 4 {
                t = sub_word(t);
            }
            enc[i] = enc[i - nk] ^ t;
        }
        let mut dec = [0u32; 60];
        for r in 0..=rounds {
            for c in 0..4 {
                let w = enc[4 * (rounds - r) + c];
                dec[4 * r + c] = if r == 0 || r == rounds {
                    w
                } else {
                    // InvMixColumns: TD holds it after InvSubBytes, which SBOX undoes.
                    let b = w.to_be_bytes().map(|b| SBOX[usize::from(b)]);
                    column(&TD, b, b, b, b, 0)
                };
            }
        }
        Ok(Aes { enc, dec, rounds })
    }

    /// Its rounds: 10, 12 or 14 (AES-128, AES-192, AES-256).
    pub fn rounds(&self) -> usize {
        self.rounds
    }

    /// Encrypts one block in place: AES itself, under no mode.
    pub fn encrypt_block(&self, block: &mut [u8; 16]) {
        let k = &self.enc;
        let mut s = load(block, &k[..4]);
        for r in 1..self.rounds {
            let [a, b, c, d] = s.map(u32::to_be_bytes);
            let k = &k[4 * r..4 * r + 4];
            s = [
                column(&TE, a, b, c, d, k[0]),
                column(&TE, b, c, d, a, k[1]),
                column(&TE, c, d, a, b, k[2]),
                column(&TE, d, a, b, c, k[3]),
            ];
        }
        let [a, b, c, d] = s.map(u32::to_be_bytes);
        let k = &k[4 * self.rounds..4 * self.rounds + 4];
        store(
            block,
            [
                last_column(&SBOX, a, b, c, d, k[0]),
                last_column(&SBOX, b, c, d, a, k[1]),
                last_column(&SBOX, c, d, a, b, k[2]),
                last_column(&SBOX, d, a, b, c, k[3]),
            ],
        );
    }

    /// Decrypts one block in place.
    pub fn decrypt_block(&self, block: &mut [u8; 16]) {
        let k = &self.dec;
        let mut s = load(block, &k[..4]);
        for r in 1..self.rounds {
            let [a, b, c, d] = s.map(u32::to_be_bytes);
            let k = &k[4 * r..4 * r + 4];
            s = [
                column(&TD, a, d, c, b, k[0]),
                column(&TD, b, a, d, c, k[1]),
                column(&TD, c, b, a, d, k[2]),
                column(&TD, d, c, b, a, k[3]),
            ];
        }
        let [a, b, c, d] = s.map(u32::to_be_bytes);
        let k = &k[4 * self.rounds..4 * self.rounds + 4];
        store(
            block,
            [
                last_column(&INV_SBOX, a, d, c, b, k[0]),
                last_column(&INV_SBOX, b, a, d, c, k[1]),
                last_column(&INV_SBOX, c, b, a, d, k[2]),
                last_column(&INV_SBOX, d, c, b, a, k[3]),
            ],
        );
    }
}

/// The block as four big-endian columns, XORed with the first round key.
fn load(block: &[u8; 16], key: &[u32]) -> [u32; 4] {
    let cols = block.as_chunks::<4>().0;
    std::array::from_fn(|i| u32::from_be_bytes(cols[i]) ^ key[i])
}

fn store(block: &mut [u8; 16], s: [u32; 4]) {
    for (b, w) in block.as_chunks_mut::<4>().0.iter_mut().zip(s) {
        *b = w.to_be_bytes();
    }
}

fn xor(data: &mut [u8], with: &[u8]) {
    for (d, w) in data.iter_mut().zip(with) {
        *d ^= w;
    }
}

/// A block cipher mode, as `crypt.encrypt` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Each block alone (PKCS#7 padded). Equal blocks encrypt equally: avoid.
    Ecb,
    /// Cipher block chaining (PKCS#7 padded).
    Cbc,
    /// Cipher feedback, 128 bits at a time (CFB-128), unpadded.
    Cfb,
    /// Output feedback, unpadded.
    Ofb,
    /// Counter: the IV is the first counter block, counted up as one 128-bit number. Unpadded.
    Ctr,
    /// Galois/counter mode: CTR with a 16-byte tag that detects any change.
    Gcm,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Ecb, Mode::Cbc, Mode::Cfb, Mode::Ofb, Mode::Ctr, Mode::Gcm];

    /// By name, in any case: "CBC", "gcm".
    pub fn from_name(name: &str) -> Option<Mode> {
        Mode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name.trim()))
    }

    /// Its name in capitals: "CBC".
    pub fn name(self) -> &'static str {
        match self {
            Mode::Ecb => "ECB",
            Mode::Cbc => "CBC",
            Mode::Cfb => "CFB",
            Mode::Ofb => "OFB",
            Mode::Ctr => "CTR",
            Mode::Gcm => "GCM",
        }
    }

    /// The IV it takes, in bytes: none for ECB, a 12-byte nonce for GCM
    /// (the usual length; others work too) and 16 for the rest.
    pub fn iv_size(self) -> usize {
        match self {
            Mode::Ecb => 0,
            Mode::Gcm => 12,
            _ => 16,
        }
    }
}

/// Encrypts with any mode. ECB takes no IV (one given is ignored); GCM's
/// output is the ciphertext, then its 16-byte tag (no additional data: for
/// that, `gcm_encrypt`).
pub fn encrypt(mode: Mode, key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match mode {
        Mode::Ecb => ecb_encrypt(key, data),
        Mode::Cbc => cbc_encrypt(key, iv, data),
        Mode::Cfb => cfb_encrypt(key, iv, data),
        Mode::Ofb => ofb_encrypt(key, iv, data),
        Mode::Ctr => ctr_encrypt(key, iv, data),
        Mode::Gcm => {
            let (mut out, tag) = gcm_encrypt(key, iv, &[], data)?;
            out.extend_from_slice(&tag);
            Ok(out)
        }
    }
}

/// Decrypts what `encrypt` made; for GCM, `data` ends with the tag, which
/// must match.
pub fn decrypt(mode: Mode, key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match mode {
        Mode::Ecb => ecb_decrypt(key, data),
        Mode::Cbc => cbc_decrypt(key, iv, data),
        Mode::Cfb => cfb_decrypt(key, iv, data),
        Mode::Ofb => ofb_decrypt(key, iv, data),
        Mode::Ctr => ctr_decrypt(key, iv, data),
        Mode::Gcm => {
            let Some(split) = data.len().checked_sub(16) else {
                return Err(format!(
                    "GCM data ends with its 16-byte tag (this is {} bytes)",
                    data.len()
                ));
            };
            gcm_decrypt(key, iv, &[], &data[..split], &data[split..])
        }
    }
}

fn iv16(mode: &str, iv: &[u8]) -> Result<[u8; 16], String> {
    match iv.len() {
        16 => Ok(array(iv)),
        n => Err(format!("{mode} needs a 16-byte IV (this one is {n} bytes)")),
    }
}

/// PKCS#7: 1 to 16 bytes, each the count, up to a whole block.
fn pad(data: &[u8]) -> Vec<u8> {
    let n = 16 - data.len() % 16;
    let mut out = Vec::with_capacity(data.len() + n);
    out.extend_from_slice(data);
    out.resize(data.len() + n, n as u8);
    out
}

fn unpad(mut data: Vec<u8>) -> Result<Vec<u8>, String> {
    let n = usize::from(*data.last().unwrap_or(&0));
    if n == 0 || n > 16 || n > data.len() || data[data.len() - n..].iter().any(|&b| usize::from(b) != n) {
        return Err("the padding is wrong: a wrong key or IV, or the data was changed".into());
    }
    data.truncate(data.len() - n);
    Ok(data)
}

fn whole_blocks(mode: &str, data: &[u8]) -> Result<(), String> {
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(format!(
            "{mode} data is whole 16-byte blocks, at least one (this is {} bytes)",
            data.len()
        ));
    }
    Ok(())
}

/// AES-ECB with PKCS#7 padding.
pub fn ecb_encrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut out = pad(data);
    for block in out.as_chunks_mut::<16>().0 {
        aes.encrypt_block(block);
    }
    Ok(out)
}

/// Undoes `ecb_encrypt`. A wrong key nearly always shows as wrong padding.
pub fn ecb_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    whole_blocks("ECB", data)?;
    let mut out = data.to_vec();
    for block in out.as_chunks_mut::<16>().0 {
        aes.decrypt_block(block);
    }
    unpad(out)
}

/// AES-CBC with PKCS#7 padding and a 16-byte IV.
pub fn cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut prev = iv16("CBC", iv)?;
    let mut out = pad(data);
    for block in out.as_chunks_mut::<16>().0 {
        xor(block, &prev);
        aes.encrypt_block(block);
        prev = *block;
    }
    Ok(out)
}

/// Undoes `cbc_encrypt`. A wrong key nearly always shows as wrong padding.
pub fn cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut prev = iv16("CBC", iv)?;
    whole_blocks("CBC", data)?;
    let mut out = data.to_vec();
    for block in out.as_chunks_mut::<16>().0 {
        let cipher = *block;
        aes.decrypt_block(block);
        xor(block, &prev);
        prev = cipher;
    }
    unpad(out)
}

/// AES-CFB-128 with a 16-byte IV; the output is as long as the input.
pub fn cfb_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut reg = iv16("CFB", iv)?;
    let mut out = data.to_vec();
    for chunk in out.chunks_mut(16) {
        aes.encrypt_block(&mut reg);
        xor(chunk, &reg);
        reg[..chunk.len()].copy_from_slice(chunk);
    }
    Ok(out)
}

/// Undoes `cfb_encrypt`.
pub fn cfb_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut reg = iv16("CFB", iv)?;
    let mut out = data.to_vec();
    for chunk in out.chunks_mut(16) {
        let mut next = reg;
        next[..chunk.len()].copy_from_slice(chunk);
        aes.encrypt_block(&mut reg);
        xor(chunk, &reg);
        reg = next;
    }
    Ok(out)
}

/// AES-OFB with a 16-byte IV; the output is as long as the input.
pub fn ofb_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut reg = iv16("OFB", iv)?;
    let mut out = data.to_vec();
    for chunk in out.chunks_mut(16) {
        aes.encrypt_block(&mut reg);
        xor(chunk, &reg);
    }
    Ok(out)
}

/// OFB is its own inverse.
pub fn ofb_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    ofb_encrypt(key, iv, data)
}

/// AES-CTR: the 16-byte IV is the first counter block, counted up as one
/// big-endian 128-bit number (as NIST SP 800-38A's examples do). The output
/// is as long as the input.
pub fn ctr_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let aes = Aes::new(key)?;
    let mut counter = u128::from_be_bytes(iv16("CTR", iv)?);
    let mut out = data.to_vec();
    for chunk in out.chunks_mut(16) {
        let mut ks = counter.to_be_bytes();
        aes.encrypt_block(&mut ks);
        xor(chunk, &ks);
        counter = counter.wrapping_add(1);
    }
    Ok(out)
}

/// CTR is its own inverse.
pub fn ctr_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    ctr_encrypt(key, iv, data)
}

/// AES-GCM (NIST SP 800-38D): the ciphertext (as long as `data`) and a
/// 16-byte tag over it and `aad`, additional data that is authenticated but
/// not encrypted. The nonce is usually 12 bytes, and must never repeat
/// under one key.
pub fn gcm_encrypt(key: &[u8], nonce: &[u8], aad: &[u8], data: &[u8]) -> Result<(Vec<u8>, [u8; 16]), String> {
    let gcm = Gcm::new(key, nonce, data.len())?;
    let mut out = data.to_vec();
    gcm.crypt(&mut out);
    let tag = gcm.tag(aad, &out);
    Ok((out, tag))
}

/// The plaintext, only if `tag` matches the ciphertext and `aad`; otherwise
/// an error, and nothing decrypted is returned.
pub fn gcm_decrypt(key: &[u8], nonce: &[u8], aad: &[u8], data: &[u8], tag: &[u8]) -> Result<Vec<u8>, String> {
    let gcm = Gcm::new(key, nonce, data.len())?;
    if tag.len() != 16 {
        return Err(format!("a GCM tag is 16 bytes (this one is {})", tag.len()));
    }
    if !same(&gcm.tag(aad, data), tag) {
        return Err("the GCM tag doesn't match: a wrong key or nonce, or the data was changed".into());
    }
    let mut out = data.to_vec();
    gcm.crypt(&mut out);
    Ok(out)
}

/// Equal, in time that depends only on the length.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && std::hint::black_box(a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y))) == 0
}

struct Gcm {
    aes: Aes,
    ghash: Ghash,
    /// The pre-counter block: its encryption masks the tag, the data's
    /// counters follow it.
    j0: u128,
}

impl Gcm {
    fn new(key: &[u8], nonce: &[u8], len: usize) -> Result<Gcm, String> {
        let aes = Aes::new(key)?;
        if nonce.is_empty() {
            return Err("GCM needs a nonce (12 bytes is usual)".into());
        }
        // 2^32 - 2 blocks, the most one counter covers.
        if len as u64 > (1u64 << 36) - 32 {
            return Err("GCM encrypts at most 64 GiB at once".into());
        }
        let mut h = [0u8; 16];
        aes.encrypt_block(&mut h);
        let ghash = Ghash::new(u128::from_be_bytes(h));
        let j0 = if nonce.len() == 12 {
            let mut b = [0u8; 16];
            b[..12].copy_from_slice(nonce);
            b[15] = 1;
            u128::from_be_bytes(b)
        } else {
            let mut y = 0;
            ghash.absorb(&mut y, nonce);
            ghash.mul(y ^ (nonce.len() as u128 * 8))
        };
        Ok(Gcm { aes, ghash, j0 })
    }

    /// CTR from the block after J0, counting in its last 32 bits.
    fn crypt(&self, data: &mut [u8]) {
        let mut counter = self.j0;
        for chunk in data.chunks_mut(16) {
            counter = (counter & !0xffff_ffff) | u128::from((counter as u32).wrapping_add(1));
            let mut ks = counter.to_be_bytes();
            self.aes.encrypt_block(&mut ks);
            xor(chunk, &ks);
        }
    }

    fn tag(&self, aad: &[u8], cipher: &[u8]) -> [u8; 16] {
        let mut y = 0;
        self.ghash.absorb(&mut y, aad);
        self.ghash.absorb(&mut y, cipher);
        let y = self
            .ghash
            .mul(y ^ ((aad.len() as u128 * 8) << 64) ^ (cipher.len() as u128 * 8));
        let mut mask = self.j0.to_be_bytes();
        self.aes.encrypt_block(&mut mask);
        (u128::from_be_bytes(mask) ^ y).to_be_bytes()
    }
}

/// x^128 = x^7 + x^2 + x + 1, in GCM's bit order (x^0 is the top bit).
const GCM_R: u128 = 0xe1 << 120;

/// What multiplying by x^4 pushes out of the low 4 bits comes back as this.
const GCM_R4: [u128; 16] = {
    let mut t = [0u128; 16];
    let mut i = 0;
    while i < 16 {
        let mut p = 0;
        while p < 4 {
            if i & (1 << p) != 0 {
                t[i] ^= GCM_R >> (3 - p);
            }
            p += 1;
        }
        i += 1;
    }
    t
};

/// GHASH's multiplication by H in GF(2^128), four bits at a time (Shoup's
/// method): `table[n]` is H times the four-bit polynomial n.
struct Ghash {
    table: [u128; 16],
}

impl Ghash {
    fn new(h: u128) -> Ghash {
        let mulx = |v: u128| (v >> 1) ^ if v & 1 == 1 { GCM_R } else { 0 };
        let mut table = [0u128; 16];
        // The top bit of a nibble is its x^0.
        table[8] = h;
        table[4] = mulx(table[8]);
        table[2] = mulx(table[4]);
        table[1] = mulx(table[2]);
        for i in 2..16usize {
            if !i.is_power_of_two() {
                let high = 1 << (usize::BITS - 1 - i.leading_zeros());
                table[i] = table[high] ^ table[i ^ high];
            }
        }
        Ghash { table }
    }

    /// x times H, by Horner's rule from x's highest powers (its low bits).
    fn mul(&self, x: u128) -> u128 {
        let mut z = 0;
        for j in 0..32 {
            z = (z >> 4) ^ GCM_R4[(z & 15) as usize];
            z ^= self.table[((x >> (4 * j)) & 15) as usize];
        }
        z
    }

    /// GHASH over `data` in 16-byte blocks, the last padded with zeros.
    fn absorb(&self, y: &mut u128, data: &[u8]) {
        let (blocks, rest) = data.as_chunks::<16>();
        for b in blocks {
            *y = self.mul(*y ^ u128::from_be_bytes(*b));
        }
        if !rest.is_empty() {
            let mut b = [0u8; 16];
            b[..rest.len()].copy_from_slice(rest);
            *y = self.mul(*y ^ u128::from_be_bytes(b));
        }
    }
}

// ---------------------------------------------------------------- random

const CHACHA_SIGMA: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

fn quarter(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(16);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(12);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(8);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(7);
}

/// One 64-byte block of the ChaCha20 keystream (RFC 8439).
pub fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut input = [0u32; 16];
    input[..4].copy_from_slice(&CHACHA_SIGMA);
    for (w, k) in input[4..12].iter_mut().zip(key.as_chunks::<4>().0) {
        *w = u32::from_le_bytes(*k);
    }
    input[12] = counter;
    for (w, n) in input[13..].iter_mut().zip(nonce.as_chunks::<4>().0) {
        *w = u32::from_le_bytes(*n);
    }
    let mut x = input;
    for _ in 0..10 {
        quarter(&mut x, 0, 4, 8, 12);
        quarter(&mut x, 1, 5, 9, 13);
        quarter(&mut x, 2, 6, 10, 14);
        quarter(&mut x, 3, 7, 11, 15);
        quarter(&mut x, 0, 5, 10, 15);
        quarter(&mut x, 1, 6, 11, 12);
        quarter(&mut x, 2, 7, 8, 13);
        quarter(&mut x, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for ((o, w), i) in out.as_chunks_mut::<4>().0.iter_mut().zip(x).zip(input) {
        *o = w.wrapping_add(i).to_le_bytes();
    }
    out
}

/// Random bytes: a ChaCha20 keystream under a 32-byte seed. `from_seed`
/// gives the same bytes for the same seed (tests, replays); `new` seeds
/// from the clock, a counter, and std's per-thread hash keys (which std
/// takes from the operating system), hashed together with SHA-256.
#[derive(Clone)]
pub struct Rng {
    key: [u8; 32],
    /// Blocks made so far: the low half is ChaCha20's counter, the high
    /// half the nonce's first word.
    block: u64,
    buf: [u8; 64],
    used: usize,
}

impl Rng {
    pub fn from_seed(seed: [u8; 32]) -> Rng {
        Rng {
            key: seed,
            block: 0,
            buf: [0; 64],
            used: 64,
        }
    }

    pub fn new() -> Rng {
        Rng::from_seed(fresh_seed())
    }

    pub fn fill(&mut self, out: &mut [u8]) {
        let mut done = 0;
        while done < out.len() {
            if self.used == 64 {
                let mut nonce = [0u8; 12];
                nonce[..4].copy_from_slice(&((self.block >> 32) as u32).to_le_bytes());
                self.buf = chacha20_block(&self.key, self.block as u32, &nonce);
                self.block = self.block.wrapping_add(1);
                self.used = 0;
            }
            let n = (64 - self.used).min(out.len() - done);
            out[done..done + n].copy_from_slice(&self.buf[self.used..self.used + n]);
            self.used += n;
            done += n;
        }
    }

    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        let mut out = vec![0; n];
        self.fill(&mut out);
        out
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill(&mut b);
        u32::from_le_bytes(b)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill(&mut b);
        u64::from_le_bytes(b)
    }
}

impl Default for Rng {
    fn default() -> Rng {
        Rng::new()
    }
}

/// A seed no other call returns: the time, a counter, the process, the
/// thread and std's hash keys, through SHA-256.
fn fresh_seed() -> [u8; 32] {
    static SEEDS: AtomicU64 = AtomicU64::new(0);
    let n = SEEDS.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let mut h = sha2::Sha256::new();
    h.update(now.to_le_bytes());
    h.update(n.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(format!("{:?}", std::thread::current().id()));
    // Where this frame is, which address space layout randomization moves.
    h.update((&raw const h as usize).to_le_bytes());
    for i in 0..4u64 {
        h.update(RandomState::new().hash_one((i, n, now)).to_le_bytes());
    }
    array(&h.finalize())
}

thread_local! {
    static RNG: RefCell<Rng> = RefCell::new(Rng::new());
}

/// `n` random bytes, from this thread's `Rng`.
pub fn random_bytes(n: usize) -> Vec<u8> {
    RNG.with(|r| r.borrow_mut().bytes(n))
}

// ---------------------------------------------------------------- LZ4

const MINMATCH: usize = 4;
/// A match starts at least this far before the end of the block...
const MFLIMIT: usize = 12;
/// ...and the last this many bytes are literals.
const LASTLITERALS: usize = 5;
const MAX_DISTANCE: usize = 65_535;
const HASH_LOG: u32 = 13;
/// Each 64 bytes without a match, the search steps one byte further.
const SKIP_TRIGGER: u32 = 6;

/// The most `lz4_compress` writes for `len` bytes (LZ4_COMPRESSBOUND).
pub fn lz4_bound(len: usize) -> usize {
    len + len / 255 + 16
}

fn read32(data: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(array(&data[i..i + 4]))
}

/// The hash table's slot for the 5 bytes at `i` (the reference's hash on
/// 64-bit machines: longer than a match's 4, so a slot's last position is
/// likelier to match far). Positions hashed are at most `data.len() - 12`.
fn lz4_hash(data: &[u8], i: usize) -> usize {
    let v = u64::from_le_bytes(array(&data[i..i + 8]));
    ((v << 24).wrapping_mul(889_523_592_379) >> (64 - HASH_LOG)) as usize
}

/// How many bytes from `a` repeat those from `b` (b < a), up to `limit`.
fn common(data: &[u8], a: usize, b: usize, limit: usize) -> usize {
    let mut n = 0;
    while a + n + 8 <= limit {
        let diff =
            u64::from_le_bytes(array(&data[a + n..a + n + 8])) ^ u64::from_le_bytes(array(&data[b + n..b + n + 8]));
        if diff != 0 {
            return n + (diff.trailing_zeros() / 8) as usize;
        }
        n += 8;
    }
    while a + n < limit && data[a + n] == data[b + n] {
        n += 1;
    }
    n
}

/// A length past a token's 15: 255s, then the rest.
fn push_len(out: &mut Vec<u8>, mut n: usize) {
    while n >= 255 {
        out.push(255);
        n -= 255;
    }
    out.push(n as u8);
}

/// A sequence: literals, then a match `offset` back of `len` bytes (none for
/// the block's last).
fn push_sequence(out: &mut Vec<u8>, literals: &[u8], back: Option<(usize, usize)>) {
    let lit = literals.len();
    let ml = back.map_or(0, |(_, len)| len - MINMATCH);
    out.push(((lit.min(15) as u8) << 4) | ml.min(15) as u8);
    if lit >= 15 {
        push_len(out, lit - 15);
    }
    out.extend_from_slice(literals);
    if let Some((offset, _)) = back {
        out.extend_from_slice(&(offset as u16).to_le_bytes());
        if ml >= 15 {
            push_len(out, ml - 15);
        }
    }
}

/// LZ4's block format, as LZ4_compress_default writes it and
/// LZ4_decompress_safe reads it: no frame, no header, no checksum, so
/// whoever decompresses needs the size (`data.len()`) from elsewhere. Empty
/// input gives one zero byte.
pub fn lz4_compress(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    let mut out = Vec::with_capacity(lz4_bound(n));
    if n < MFLIMIT + 1 {
        push_sequence(&mut out, data, None);
        return out;
    }
    // A match starts at or before `mf_limit` and ends at or before `match_limit`.
    let mf_limit = n - MFLIMIT;
    let match_limit = n - LASTLITERALS;
    let mut table = vec![0usize; 1 << HASH_LOG];
    let found = |table: &mut [usize], ip: usize| {
        let h = lz4_hash(data, ip);
        let cand = table[h];
        table[h] = ip;
        (cand < ip && ip - cand <= MAX_DISTANCE && read32(data, cand) == read32(data, ip)).then_some(cand)
    };
    let (mut anchor, mut ip) = (0, 0);
    'search: loop {
        let mut tries = 1 << SKIP_TRIGGER;
        let mut cand = loop {
            if ip > mf_limit {
                break 'search;
            }
            if let Some(c) = found(&mut table, ip) {
                break c;
            }
            ip += tries >> SKIP_TRIGGER;
            tries += 1;
        };
        // The match may begin earlier, in bytes that would be literals.
        while ip > anchor && cand > 0 && data[ip - 1] == data[cand - 1] {
            ip -= 1;
            cand -= 1;
        }
        // Matches, back to back while each ends where another begins.
        loop {
            let len = MINMATCH + common(data, ip + MINMATCH, cand + MINMATCH, match_limit);
            push_sequence(&mut out, &data[anchor..ip], Some((ip - cand, len)));
            ip += len;
            anchor = ip;
            if ip > mf_limit {
                break 'search;
            }
            table[lz4_hash(data, ip - 2)] = ip - 2;
            match found(&mut table, ip) {
                Some(c) => cand = c,
                None => {
                    ip += 1;
                    continue 'search;
                }
            }
        }
    }
    push_sequence(&mut out, &data[anchor..], None);
    out
}

/// Reads an LZ4 block, as LZ4_decompress_safe does: `size` is the most it
/// may decompress to (the original length, usually), and a block that
/// would make more is an error, as is one that is cut short, reaches back
/// before its start or doesn't end with literals. Memory grows with what
/// the block really holds, never with `size` alone.
pub fn lz4_decompress(block: &[u8], size: usize) -> Result<Vec<u8>, String> {
    if block.is_empty() {
        return Err("an LZ4 block is at least one byte (nothing compresses to a single 0)".into());
    }
    let cut = || "the LZ4 block is cut short".to_string();
    let too_big = || format!("the LZ4 block decompresses to more than {size} bytes");
    let mut out: Vec<u8> = Vec::with_capacity(size.min(block.len().saturating_mul(4)));
    let mut i = 0;
    // At the top of each sequence, i < block.len().
    loop {
        let token = block[i];
        i += 1;
        let lit = lz4_len(block, &mut i, usize::from(token >> 4)).ok_or_else(cut)?;
        let end = i.checked_add(lit).filter(|&e| e <= block.len()).ok_or_else(cut)?;
        if lit > size - out.len() {
            return Err(too_big());
        }
        out.extend_from_slice(&block[i..end]);
        i = end;
        if i == block.len() {
            return Ok(out);
        }
        if block.len() - i < 2 {
            return Err(cut());
        }
        let offset = usize::from(u16::from_le_bytes([block[i], block[i + 1]]));
        i += 2;
        if offset == 0 || offset > out.len() {
            return Err(format!(
                "the LZ4 block is corrupt: a match {offset} bytes back, {} bytes in",
                out.len()
            ));
        }
        let len = lz4_len(block, &mut i, usize::from(token & 15))
            .ok_or_else(cut)?
            .saturating_add(MINMATCH);
        if len > size - out.len() {
            return Err(too_big());
        }
        repeat(&mut out, offset, len);
        if i == block.len() {
            return Err("the LZ4 block ends with a match, not literals".into());
        }
    }
}

/// A token's length (`nibble`) with the bytes after it that a 15 calls for;
/// None if the block ends first.
fn lz4_len(block: &[u8], i: &mut usize, nibble: usize) -> Option<usize> {
    let mut len = nibble;
    if nibble == 15 {
        loop {
            let b = *block.get(*i)?;
            *i += 1;
            len = len.saturating_add(usize::from(b));
            if b != 255 {
                break;
            }
        }
    }
    Some(len)
}

/// Appends `len` bytes copied from `offset` back, which may overlap what
/// they write (offset 1 repeats the last byte): copied in runs that double,
/// each a whole number of the repeating pattern.
fn repeat(out: &mut Vec<u8>, offset: usize, len: usize) {
    out.reserve(len);
    let start = out.len() - offset;
    let mut left = len;
    while left > 0 {
        let n = left.min(out.len() - start);
        out.extend_from_within(start..start + n);
        left -= n;
    }
}
