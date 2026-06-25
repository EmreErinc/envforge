//! Coverage for `ops::canary::v2` forensic token codec: HMAC round-trip,
//! forgery detection (wrong key), multi-key resolution, format errors, and
//! timestamp/age math.

use chrono::{DateTime, Utc};
use envforge::ops::canary::v2::{
    decode_token, encode_token, V2Error, V2Payload, CANARY_EPOCH_UNIX_SECS, V2_PREFIX,
};

fn payload() -> V2Payload {
    V2Payload {
        machine_id: [1, 2, 3, 4, 5, 6, 7, 8],
        pid: 12345,
        timestamp_secs: 100,
        agent_name_hash: [9, 10, 11, 12],
        key_name_hash: [13, 14, 15, 16],
    }
}

#[test]
fn test_encode_decode_roundtrip_valid_key() {
    let key = [0x42u8; 32];
    let token = encode_token(&payload(), &key);
    assert!(token.starts_with(V2_PREFIX));

    let decoded = decode_token(&token, vec![(1u8, &key)]).unwrap();
    assert!(decoded.hmac_valid);
    assert_eq!(decoded.version, 2);
    assert_eq!(decoded.key_version_used, Some(1));
    assert_eq!(decoded.payload, Some(payload()));
}

#[test]
fn test_decode_wrong_key_surfaces_payload_but_invalid() {
    let key = [0x42u8; 32];
    let wrong = [0x99u8; 32];
    let token = encode_token(&payload(), &key);

    let decoded = decode_token(&token, vec![(0u8, &wrong)]).unwrap();
    assert!(
        !decoded.hmac_valid,
        "forged/mismatched key must not validate"
    );
    assert_eq!(decoded.key_version_used, None);
    // Payload fields are still surfaced for forensic value.
    assert_eq!(decoded.payload, Some(payload()));
}

#[test]
fn test_decode_tries_multiple_keys() {
    let key = [0x42u8; 32];
    let wrong = [0x01u8; 32];
    let token = encode_token(&payload(), &key);
    let decoded = decode_token(&token, vec![(0u8, &wrong), (1u8, &key)]).unwrap();
    assert!(decoded.hmac_valid);
    assert_eq!(decoded.key_version_used, Some(1));
}

#[test]
fn test_decode_non_v2_token_errors() {
    let key = [0u8; 32];
    assert!(matches!(
        decode_token("not-a-canary", vec![(0u8, &key)]),
        Err(V2Error::NotV2)
    ));
}

#[test]
fn test_decode_malformed_v2_token_errors() {
    let key = [0u8; 32];
    assert!(matches!(
        decode_token("cnry_tooshort_x", vec![(0u8, &key)]),
        Err(V2Error::BadTokenFormat)
    ));
}

#[test]
fn test_payload_timestamp_and_age() {
    let p = payload();
    assert_eq!(p.timestamp_unix(), CANARY_EPOCH_UNIX_SECS + 100);

    let now = DateTime::<Utc>::from_timestamp((CANARY_EPOCH_UNIX_SECS + 300) as i64, 0).unwrap();
    assert_eq!(p.age_seconds(now), 200);
}

#[test]
fn test_payload_new_derives_timestamp_from_now() {
    let now = DateTime::<Utc>::from_timestamp((CANARY_EPOCH_UNIX_SECS + 50) as i64, 0).unwrap();
    let p = V2Payload::new([0u8; 8], 7, now, "claude", "API_KEY");
    assert_eq!(p.timestamp_secs, 50);
    assert_eq!(p.timestamp_unix(), CANARY_EPOCH_UNIX_SECS + 50);
}
