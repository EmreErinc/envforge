//! Coverage for `ops::ci_trust::classifier` — the pure GitHub Actions trigger
//! trust matrix that decides whether secrets may be exposed to a workflow run.
//! Fork PRs and external comments must classify as Untrusted.

use envforge::ops::ci_trust::classifier::{
    classify, AuthorAssociation, TriggerContext, TrustLevel, TrustReason, CLASSIFIER_VERSION,
};

fn ctx(event: &str) -> TriggerContext {
    TriggerContext {
        event_name: event.to_string(),
        ..Default::default()
    }
}

// ---- AuthorAssociation -----------------------------------------------------

#[test]
fn test_author_association_parse_and_trust() {
    assert_eq!(AuthorAssociation::parse("OWNER"), AuthorAssociation::Owner);
    assert_eq!(
        AuthorAssociation::parse("MEMBER"),
        AuthorAssociation::Member
    );
    assert_eq!(AuthorAssociation::parse("bogus"), AuthorAssociation::None);

    assert!(AuthorAssociation::Owner.is_trusted());
    assert!(AuthorAssociation::Member.is_trusted());
    assert!(AuthorAssociation::Collaborator.is_trusted());
    assert!(!AuthorAssociation::Contributor.is_trusted());
    assert!(!AuthorAssociation::FirstTimeContributor.is_trusted());
    assert!(!AuthorAssociation::None.is_trusted());
}

// ---- classify: trusted events ----------------------------------------------

#[test]
fn test_classify_trusted_events() {
    for ev in ["push", "workflow_dispatch", "schedule"] {
        let v = classify(&ctx(ev));
        assert_eq!(v.level, TrustLevel::Trusted, "{ev} should be trusted");
        assert_eq!(v.classifier_version, CLASSIFIER_VERSION);
    }
}

// ---- classify: pull_request fork logic (security core) ---------------------

#[test]
fn test_classify_fork_pr_is_untrusted() {
    let mut c = ctx("pull_request");
    c.pr_head_repo_fork = Some(true);
    let v = classify(&c);
    assert_eq!(v.level, TrustLevel::Untrusted);
    assert_eq!(v.reason, TrustReason::ForkPr);
}

#[test]
fn test_classify_internal_pr_is_trusted() {
    let mut c = ctx("pull_request");
    c.pr_head_repo_fork = Some(false);
    let v = classify(&c);
    assert_eq!(v.level, TrustLevel::Trusted);
    assert_eq!(v.reason, TrustReason::InternalPr);
}

#[test]
fn test_classify_pr_unknown_fork_is_untrusted() {
    let c = ctx("pull_request"); // pr_head_repo_fork = None
    let v = classify(&c);
    assert_eq!(v.level, TrustLevel::Untrusted);
    assert_eq!(v.reason, TrustReason::Unknown);
}

#[test]
fn test_classify_pull_request_target_untrusted() {
    let v = classify(&ctx("pull_request_target"));
    assert_eq!(v.level, TrustLevel::Untrusted);
    assert_eq!(v.reason, TrustReason::PullRequestTarget);
}

// ---- classify: issue_comment by author association -------------------------

#[test]
fn test_classify_issue_comment_by_author() {
    let mut trusted = ctx("issue_comment");
    trusted.comment_author_assoc = Some(AuthorAssociation::Owner);
    assert_eq!(classify(&trusted).level, TrustLevel::Trusted);

    let mut external = ctx("issue_comment");
    external.comment_author_assoc = Some(AuthorAssociation::Contributor);
    let v = classify(&external);
    assert_eq!(v.level, TrustLevel::Untrusted);
    assert_eq!(v.reason, TrustReason::ExternalComment);
}

// ---- classify: fail-closed defaults ----------------------------------------

#[test]
fn test_classify_unknown_and_empty_fail_closed() {
    assert_eq!(classify(&ctx("")).level, TrustLevel::Untrusted);
    assert_eq!(
        classify(&ctx("totally_new_event")).level,
        TrustLevel::Untrusted
    );
    assert_eq!(
        classify(&ctx("workflow_run")).reason,
        TrustReason::WorkflowRunChain
    );
    assert_eq!(classify(&ctx("workflow_run")).level, TrustLevel::Untrusted);
}
