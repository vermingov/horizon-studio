//! The voice packets' encryption and framing (`Secret.java`,
//! `NetworkMessage.java`): AES-128-GCM with the secret's 16 bytes as the key,
//! a fresh 12-byte IV per packet put in front, a 128-bit tag after, no
//! associated data. A datagram to the server is `0xFF | UUID | ByteArray`,
//! one from it `0xFF | ByteArray`.

use crate::proto::{MAGIC, MAX_PACKET, Reader, Writer};
use ring::aead::{AES_128_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};

/// One connection's key.
pub struct Cipher {
    key: LessSafeKey,
    rng: SystemRandom,
}

impl Cipher {
    pub fn new(secret: &[u8; 16]) -> Cipher {
        let key = UnboundKey::new(&AES_128_GCM, secret).expect("AES-128 takes a 16-byte key");
        Cipher {
            key: LessSafeKey::new(key),
            rng: SystemRandom::new(),
        }
    }

    /// `IV ‖ ciphertext ‖ tag`, with a random IV.
    pub fn encrypt(&self, plain: &[u8]) -> Vec<u8> {
        let mut iv = [0u8; NONCE_LEN];
        if self.rng.fill(&mut iv).is_err() {
            // The system's generator failed: a time-based IV still differs per packet.
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            iv.copy_from_slice(&t.to_le_bytes()[..NONCE_LEN]);
        }
        self.encrypt_with_iv(iv, plain)
    }

    /// `encrypt` with a given IV (tests, and known vectors).
    pub fn encrypt_with_iv(&self, iv: [u8; NONCE_LEN], plain: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(NONCE_LEN + plain.len() + 16);
        out.extend_from_slice(&iv);
        let mut body = plain.to_vec();
        let tag = self
            .key
            .seal_in_place_separate_tag(Nonce::assume_unique_for_key(iv), Aad::empty(), &mut body)
            .expect("AES-GCM seals any packet this size");
        out.extend_from_slice(&body);
        out.extend_from_slice(tag.as_ref());
        out
    }

    /// The plaintext, or None for a packet this key didn't seal.
    pub fn decrypt(&self, payload: &[u8]) -> Option<Vec<u8>> {
        if payload.len() < NONCE_LEN + 16 {
            return None;
        }
        let mut iv = [0u8; NONCE_LEN];
        iv.copy_from_slice(&payload[..NONCE_LEN]);
        let mut body = payload[NONCE_LEN..].to_vec();
        let plain = self
            .key
            .open_in_place(Nonce::assume_unique_for_key(iv), Aad::empty(), &mut body)
            .ok()?;
        let n = plain.len();
        body.truncate(n);
        Some(body)
    }
}

/// A datagram to the server: `0xFF | id | ByteArray(encrypted)` (the id is
/// compat 21's connection id, compat 20's player UUID).
pub fn to_server(id: u128, encrypted: &[u8]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(MAGIC).uuid(id).byte_array(encrypted);
    w.finish()
}

/// A datagram from the server: `0xFF | ByteArray(encrypted)`.
pub fn to_client(encrypted: &[u8]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(MAGIC).byte_array(encrypted);
    w.finish()
}

/// The encrypted payload of a datagram from the server.
pub fn from_server(datagram: &[u8]) -> Option<Vec<u8>> {
    let mut r = Reader::new(datagram);
    if r.u8().ok()? != MAGIC {
        return None;
    }
    r.byte_array(MAX_PACKET).ok()
}

/// The id and encrypted payload of a datagram to the server (the fake
/// servers of the tests read them so).
pub fn from_client(datagram: &[u8]) -> Option<(u128, Vec<u8>)> {
    let mut r = Reader::new(datagram);
    if r.u8().ok()? != MAGIC {
        return None;
    }
    let id = r.uuid().ok()?;
    Some((id, r.byte_array(MAX_PACKET).ok()?))
}
