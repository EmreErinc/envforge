//! Regression test (2026-06-20 hardening): a JIT lease must only be redeemable
//! with the secret ticket UUID returned at grant time — not by anyone who
//! merely learns the (audit-logged, predictable) lease name.
//!
//! Single test in its own binary so the process-global `ENVFORGE_CONFIG_DIR`
//! override cannot race other tests.

use envforge::ops::lease::{jit_grant, jit_redeem, GrantRequest, JitHandle, LeaseError};

#[tokio::test]
async fn test_jit_redeem_requires_matching_ticket() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("ENVFORGE_CONFIG_DIR", tmp.path());
    std::env::set_var("ENVFORGE_JIT_SECRET", "s3cr3t-value");

    // A real, alive, non-self PID so the PID watcher does not immediately
    // revoke the lease (it rejects our own PID, and a dead PID would race).
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("spawn sleep");
    let pid = child.id();

    let handle = jit_grant(GrantRequest {
        key: "ENVFORGE_JIT_SECRET".into(),
        pid,
        ttl_secs: 3600,
        tool_name: "test".into(),
        single_redeem: true,
    })
    .expect("grant");

    // Forged handle: correct lease name, wrong ticket → rejected, and the
    // single-redeem flag is NOT consumed (the genuine redeem below still works).
    let forged = JitHandle {
        uuid: "00000000-0000-0000-0000-forged0forged".into(),
        lease_name: handle.lease_name.clone(),
    };
    assert!(
        matches!(jit_redeem(&forged), Err(LeaseError::InvalidTicket(_))),
        "forged ticket must be rejected with InvalidTicket"
    );

    // Genuine handle redeems the bound secret.
    let val = jit_redeem(&handle).expect("genuine redeem");
    assert_eq!(&*val, "s3cr3t-value");

    let _ = child.kill();
    let _ = child.wait();
}
