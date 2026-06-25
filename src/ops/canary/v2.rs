//! Forensic canary v2 — encoded token format with HMAC-SHA256 integrity.
//!
//! HMAC tag truncated to 8 bytes for log-line budget.
//!
//! Wire format: `cnry_<39-char base32>_<13-char base32>` (RFC 4648 alphabet, no pad).
//! Total length 58 chars; fits 64-char log-line budget.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

pub const V2_PREFIX: &str = "cnry_";
pub const VERSION_BYTE_V2: u8 = 0x02;
const PAYLOAD_BYTES: usize = 24;
const HMAC_TAG_BYTES: usize = 8;
const PAYLOAD_B32_LEN: usize = 39;
const HMAC_B32_LEN: usize = 13;
/// Canary epoch — 2026-01-01T00:00:00Z as Unix seconds.
/// Load-bearing constant: changing this breaks decode of every prior token.
pub const CANARY_EPOCH_UNIX_SECS: u64 = 1_767_225_600;

/// 24-byte payload encoding issuance context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V2Payload {
    pub machine_id: [u8; 8],
    pub pid: u32,
    /// Seconds since [`CANARY_EPOCH_UNIX_SECS`].
    pub timestamp_secs: u32,
    pub agent_name_hash: [u8; 4],
    pub key_name_hash: [u8; 4],
}

impl V2Payload {
    /// Build a payload from runtime context.
    pub fn new(
        machine_id: [u8; 8],
        pid: u32,
        now: DateTime<Utc>,
        tool_name: &str,
        key_name: &str,
    ) -> Self {
        let unix = u64::try_from(now.timestamp().max(0)).unwrap_or(0);
        let ts = unix.saturating_sub(CANARY_EPOCH_UNIX_SECS) as u32;
        Self {
            machine_id,
            pid,
            timestamp_secs: ts,
            agent_name_hash: short_hash(tool_name),
            key_name_hash: short_hash(key_name),
        }
    }

    fn to_bytes(self) -> [u8; PAYLOAD_BYTES] {
        let mut out = [0u8; PAYLOAD_BYTES];
        out[0..8].copy_from_slice(&self.machine_id);
        out[8..12].copy_from_slice(&self.pid.to_le_bytes());
        out[12..16].copy_from_slice(&self.timestamp_secs.to_le_bytes());
        out[16..20].copy_from_slice(&self.agent_name_hash);
        out[20..24].copy_from_slice(&self.key_name_hash);
        out
    }

    fn from_bytes(b: [u8; PAYLOAD_BYTES]) -> Self {
        let mut machine_id = [0u8; 8];
        machine_id.copy_from_slice(&b[0..8]);
        let pid = u32::from_le_bytes(b[8..12].try_into().unwrap());
        let timestamp_secs = u32::from_le_bytes(b[12..16].try_into().unwrap());
        let mut agent_name_hash = [0u8; 4];
        agent_name_hash.copy_from_slice(&b[16..20]);
        let mut key_name_hash = [0u8; 4];
        key_name_hash.copy_from_slice(&b[20..24]);
        Self {
            machine_id,
            pid,
            timestamp_secs,
            agent_name_hash,
            key_name_hash,
        }
    }

    /// Recover absolute Unix-seconds timestamp.
    pub fn timestamp_unix(&self) -> u64 {
        CANARY_EPOCH_UNIX_SECS.saturating_add(u64::from(self.timestamp_secs))
    }

    /// Age of token in seconds relative to `now`.
    pub fn age_seconds(&self, now: DateTime<Utc>) -> i64 {
        let now_unix = now.timestamp().max(0) as u64;
        i64::try_from(now_unix).unwrap_or(0) - i64::try_from(self.timestamp_unix()).unwrap_or(0)
    }
}

/// Decode result. Surfaces raw fields even on HMAC failure for forensic value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedCanary {
    pub version: u8,
    pub hmac_valid: bool,
    pub payload: Option<V2Payload>,
    pub age_seconds: Option<i64>,
    /// Index into the registry's tried key list that matched (None on failure or v1).
    pub key_version_used: Option<u8>,
    /// True for v1 (legacy) tokens — payload is None and HMAC is irrelevant.
    pub opaque: bool,
}

impl DecodedCanary {
    pub fn v1_opaque() -> Self {
        Self {
            version: 1,
            hmac_valid: false,
            payload: None,
            age_seconds: None,
            key_version_used: None,
            opaque: true,
        }
    }
}

/// First 4 bytes of SHA-256(input).
fn short_hash(s: &str) -> [u8; 4] {
    let h = Sha256::digest(s.as_bytes());
    let mut out = [0u8; 4];
    out.copy_from_slice(&h[..4]);
    out
}

// ─── Manual HMAC-SHA256 (avoids hmac crate version-pin against sha2 0.11) ──

const SHA256_BLOCK: usize = 64;
const SHA256_OUTPUT: usize = 32;

fn hmac_sha256(key: &[u8], msg1: &[u8], msg2: &[u8]) -> [u8; SHA256_OUTPUT] {
    let mut k = [0u8; SHA256_BLOCK];
    if key.len() > SHA256_BLOCK {
        let h = Sha256::digest(key);
        k[..SHA256_OUTPUT].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; SHA256_BLOCK];
    let mut opad = [0x5cu8; SHA256_BLOCK];
    for i in 0..SHA256_BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg1);
    inner.update(msg2);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    let final_digest = outer.finalize();
    let mut out = [0u8; SHA256_OUTPUT];
    out.copy_from_slice(&final_digest);
    out
}

/// Constant-time byte-slice equality (defeats timing oracles on tag compare).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

// ─── Base32 RFC 4648 (no padding) ──────────────────────────────────────────

const B32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn b32_encode(data: &[u8]) -> String {
    let bits = data.len() * 8;
    let chars = bits.div_ceil(5);
    let mut out = Vec::with_capacity(chars);
    let mut buf = 0u64;
    let mut buf_bits = 0u32;
    for &b in data {
        buf = (buf << 8) | u64::from(b);
        buf_bits += 8;
        while buf_bits >= 5 {
            buf_bits -= 5;
            let idx = ((buf >> buf_bits) & 0x1f) as usize;
            out.push(B32_ALPHABET[idx]);
        }
    }
    if buf_bits > 0 {
        let idx = ((buf << (5 - buf_bits)) & 0x1f) as usize;
        out.push(B32_ALPHABET[idx]);
    }
    String::from_utf8(out).expect("base32 alphabet is ascii")
}

fn b32_decode(input: &str, expected_bytes: usize) -> Result<Vec<u8>, V2Error> {
    let mut out = Vec::with_capacity(expected_bytes);
    let mut buf = 0u64;
    let mut buf_bits = 0u32;
    for c in input.chars() {
        let v = match c {
            'A'..='Z' => (c as u8) - b'A',
            'a'..='z' => (c as u8) - b'a',
            '2'..='7' => (c as u8) - b'2' + 26,
            _ => return Err(V2Error::BadTokenFormat),
        };
        buf = (buf << 5) | u64::from(v);
        buf_bits += 5;
        if buf_bits >= 8 {
            buf_bits -= 8;
            let byte = ((buf >> buf_bits) & 0xff) as u8;
            out.push(byte);
            if out.len() == expected_bytes {
                break;
            }
        }
    }
    if out.len() != expected_bytes {
        return Err(V2Error::BadTokenFormat);
    }
    Ok(out)
}

// ─── Encode / decode / verify ──────────────────────────────────────────────

/// Encode a v2 token using the given HMAC key.
pub fn encode_token(payload: &V2Payload, hmac_key: &[u8; 32]) -> String {
    let payload_bytes = payload.to_bytes();
    let tag = hmac_sha256(hmac_key, &payload_bytes, &[VERSION_BYTE_V2]);
    let payload_b32 = b32_encode(&payload_bytes);
    let hmac_b32 = b32_encode(&tag[..HMAC_TAG_BYTES]);
    debug_assert_eq!(payload_b32.len(), PAYLOAD_B32_LEN);
    debug_assert_eq!(hmac_b32.len(), HMAC_B32_LEN);
    format!("{V2_PREFIX}{payload_b32}_{hmac_b32}")
}

/// Decode a v2 token, trying each key from the registry in order.
/// Returns `Ok(DecodedCanary)` even on HMAC failure (fields surfaced for forensics).
/// Returns `Err` only on unparseable tokens.
pub fn decode_token<'a, K>(token: &str, candidate_keys: K) -> Result<DecodedCanary, V2Error>
where
    K: IntoIterator<Item = (u8, &'a [u8; 32])>,
{
    if !token.starts_with(V2_PREFIX) {
        // Could be v1 — caller distinguishes.
        return Err(V2Error::NotV2);
    }
    let rest = &token[V2_PREFIX.len()..];
    let parts: Vec<&str> = rest.split('_').collect();
    if parts.len() != 2 || parts[0].len() != PAYLOAD_B32_LEN || parts[1].len() != HMAC_B32_LEN {
        return Err(V2Error::BadTokenFormat);
    }
    let payload_bytes = b32_decode(parts[0], PAYLOAD_BYTES)?;
    let hmac_bytes = b32_decode(parts[1], HMAC_TAG_BYTES)?;
    let payload_arr: [u8; PAYLOAD_BYTES] = payload_bytes
        .clone()
        .try_into()
        .map_err(|_| V2Error::BadTokenFormat)?;
    let payload = V2Payload::from_bytes(payload_arr);

    for (version, key) in candidate_keys {
        let expected = hmac_sha256(key, &payload_bytes, &[VERSION_BYTE_V2]);
        if constant_time_eq(&expected[..HMAC_TAG_BYTES], &hmac_bytes) {
            let age = payload.age_seconds(Utc::now());
            return Ok(DecodedCanary {
                version: 2,
                hmac_valid: true,
                payload: Some(payload),
                age_seconds: Some(age),
                key_version_used: Some(version),
                opaque: false,
            });
        }
    }
    let age = payload.age_seconds(Utc::now());
    Ok(DecodedCanary {
        version: 2,
        hmac_valid: false,
        payload: Some(payload),
        age_seconds: Some(age),
        key_version_used: None,
        opaque: false,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum V2Error {
    #[error("token does not start with v2 prefix")]
    NotV2,
    #[error("token format is malformed")]
    BadTokenFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload() -> V2Payload {
        V2Payload {
            machine_id: [1, 2, 3, 4, 5, 6, 7, 8],
            pid: 12345,
            timestamp_secs: 100,
            agent_name_hash: [9, 10, 11, 12],
            key_name_hash: [13, 14, 15, 16],
        }
    }

    #[test]
    fn payload_roundtrip_bytes() {
        let p = sample_payload();
        let b = p.to_bytes();
        assert_eq!(b.len(), PAYLOAD_BYTES);
        let p2 = V2Payload::from_bytes(b);
        assert_eq!(p, p2);
    }

    #[test]
    fn b32_roundtrip() {
        let data = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let enc = b32_encode(&data);
        let dec = b32_decode(&enc, data.len()).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn b32_payload_length() {
        let p = sample_payload();
        let enc = b32_encode(&p.to_bytes());
        assert_eq!(enc.len(), PAYLOAD_B32_LEN);
    }

    #[test]
    fn b32_hmac_length() {
        let h = [0u8; HMAC_TAG_BYTES];
        let enc = b32_encode(&h);
        assert_eq!(enc.len(), HMAC_B32_LEN);
    }

    #[test]
    fn token_roundtrip() {
        let key = [0x42u8; 32];
        let p = sample_payload();
        let token = encode_token(&p, &key);
        assert!(token.starts_with(V2_PREFIX));
        assert_eq!(
            token.len(),
            V2_PREFIX.len() + PAYLOAD_B32_LEN + 1 + HMAC_B32_LEN
        );
        let dec = decode_token(&token, [(1u8, &key)]).unwrap();
        assert!(dec.hmac_valid);
        assert_eq!(dec.version, 2);
        assert_eq!(dec.payload, Some(p));
        assert_eq!(dec.key_version_used, Some(1));
    }

    #[test]
    fn token_total_length_within_budget() {
        let key = [0u8; 32];
        let token = encode_token(&sample_payload(), &key);
        assert!(token.len() <= 64, "token too long: {} chars", token.len());
    }

    #[test]
    fn tamper_detected() {
        let key = [0xaau8; 32];
        let p = sample_payload();
        let token = encode_token(&p, &key);
        // Flip a byte in the payload section.
        let mut bytes = token.into_bytes();
        let payload_start = V2_PREFIX.len();
        bytes[payload_start] = if bytes[payload_start] == b'A' {
            b'B'
        } else {
            b'A'
        };
        let tampered = String::from_utf8(bytes).unwrap();
        let dec = decode_token(&tampered, [(1u8, &key)]).unwrap();
        // Decoded fields surface; HMAC must fail.
        assert!(!dec.hmac_valid);
        assert!(dec.payload.is_some());
        assert_eq!(dec.key_version_used, None);
    }

    #[test]
    fn wrong_key_fails_hmac() {
        let key1 = [0x11u8; 32];
        let key2 = [0x22u8; 32];
        let token = encode_token(&sample_payload(), &key1);
        let dec = decode_token(&token, [(1u8, &key2)]).unwrap();
        assert!(!dec.hmac_valid);
    }

    #[test]
    fn retired_key_decode_succeeds() {
        let active = [0x11u8; 32];
        let retired = [0x22u8; 32];
        let token = encode_token(&sample_payload(), &retired);
        let dec = decode_token(&token, [(2u8, &active), (1u8, &retired)]).unwrap();
        assert!(dec.hmac_valid);
        assert_eq!(dec.key_version_used, Some(1));
    }

    #[test]
    fn reject_v1_token_shape() {
        let result = decode_token(
            "AKIAIOSFODNN7EXAMPLE",
            std::iter::empty::<(u8, &[u8; 32])>(),
        );
        assert!(matches!(result, Err(V2Error::NotV2)));
    }

    #[test]
    fn reject_malformed_v2_segment_count() {
        let key = [0u8; 32];
        let bad = format!("{V2_PREFIX}OOPS");
        let result = decode_token(&bad, [(1u8, &key)]);
        assert!(matches!(result, Err(V2Error::BadTokenFormat)));
    }

    #[test]
    fn deterministic_roundtrip_1000_payloads() {
        let key = [0x55u8; 32];
        for i in 0..1000u32 {
            let p = V2Payload {
                machine_id: [i as u8, 0, 0, 0, 0, 0, 0, 0],
                pid: i,
                timestamp_secs: i,
                agent_name_hash: [0, 0, 0, 0],
                key_name_hash: [0, 0, 0, 0],
            };
            let token = encode_token(&p, &key);
            let dec = decode_token(&token, [(1u8, &key)]).unwrap();
            assert!(dec.hmac_valid);
            assert_eq!(dec.payload, Some(p));
        }
    }

    #[test]
    fn epoch_constant_is_2026_01_01() {
        // 2026-01-01T00:00:00Z = 1767225600
        // sanity check the load-bearing constant
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(CANARY_EPOCH_UNIX_SECS as i64, 0)
            .unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-01-01");
    }

    #[test]
    fn v2_payload_new_uses_epoch_offset() {
        let now = chrono::DateTime::<chrono::Utc>::from_timestamp(
            CANARY_EPOCH_UNIX_SECS as i64 + 1000,
            0,
        )
        .unwrap();
        let p = V2Payload::new([0; 8], 1, now, "x", "y");
        assert_eq!(p.timestamp_secs, 1000);
    }

    #[test]
    fn hmac_known_answer() {
        // RFC 4231 Test Case 1: key=20 bytes 0x0b, data="Hi There"
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let mac = hmac_sha256(&key, data, &[]);
        // Expected: b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7
        let expected =
            hex::decode("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
                .unwrap();
        assert_eq!(&mac[..], &expected[..]);
    }

    #[test]
    fn constant_time_eq_correctness() {
        assert!(constant_time_eq(&[1, 2, 3], &[1, 2, 3]));
        assert!(!constant_time_eq(&[1, 2, 3], &[1, 2, 4]));
        assert!(!constant_time_eq(&[1, 2], &[1, 2, 3]));
    }
}
