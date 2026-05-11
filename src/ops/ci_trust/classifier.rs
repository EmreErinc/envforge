//! GitHub Actions trigger classification.
//!
//! Pure decision-matrix logic over the trigger context — no I/O during `classify`.
//! Reading the context from env + payload and caching the verdict are separate
//! steps the caller invokes explicitly.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CLASSIFIER_VERSION: u32 = 1;
const VERDICT_CACHE_FILE: &str = "envforge-trust.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Trusted,
    Suspicious,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustReason {
    Push,
    InternalPr,
    ForkPr,
    PullRequestTarget,
    ExternalComment,
    WorkflowRunChain,
    Schedule,
    WorkflowDispatch,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorAssociation {
    Owner,
    Member,
    Collaborator,
    Contributor,
    FirstTimeContributor,
    FirstTimer,
    Mannequin,
    None,
}

impl AuthorAssociation {
    pub fn is_trusted(self) -> bool {
        matches!(self, Self::Owner | Self::Member | Self::Collaborator)
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "OWNER" => Self::Owner,
            "MEMBER" => Self::Member,
            "COLLABORATOR" => Self::Collaborator,
            "CONTRIBUTOR" => Self::Contributor,
            "FIRST_TIME_CONTRIBUTOR" => Self::FirstTimeContributor,
            "FIRST_TIMER" => Self::FirstTimer,
            "MANNEQUIN" => Self::Mannequin,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerContext {
    pub event_name: String,
    pub actor: String,
    pub repo: String,
    pub head_repo_full_name: Option<String>,
    pub base_repo_full_name: Option<String>,
    pub comment_author_assoc: Option<AuthorAssociation>,
    pub pr_head_repo_fork: Option<bool>,
    pub is_workflow_run_chain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustVerdict {
    pub level: TrustLevel,
    pub reason: TrustReason,
    pub classifier_version: u32,
}

/// Pure decision-matrix classifier. No I/O.
pub fn classify(ctx: &TriggerContext) -> TrustVerdict {
    #[allow(clippy::enum_glob_use)]
    use TrustLevel::*;
    #[allow(clippy::enum_glob_use)]
    use TrustReason::*;

    let (level, reason) = match ctx.event_name.as_str() {
        "" => (Untrusted, Unknown),
        "push" => (Trusted, Push),
        "workflow_dispatch" => (Trusted, WorkflowDispatch),
        "schedule" => (Trusted, Schedule),
        "pull_request" => match ctx.pr_head_repo_fork {
            Some(true) => (Untrusted, ForkPr),
            Some(false) => (Trusted, InternalPr),
            None => (Untrusted, Unknown),
        },
        "pull_request_target" => (Untrusted, PullRequestTarget),
        "issue_comment" => match ctx.comment_author_assoc {
            Some(a) if a.is_trusted() => (Trusted, Push),
            Some(_) => (Untrusted, ExternalComment),
            None => (Untrusted, Unknown),
        },
        "workflow_run" => {
            // Without inspecting the upstream event, conservatively treat workflow_run
            // as Untrusted unless explicit signals (e.g. cached prior verdict) say
            // otherwise. Refining this is a future-intent extension.
            (Untrusted, WorkflowRunChain)
        }
        _ => (Untrusted, Unknown),
    };

    TrustVerdict {
        level,
        reason,
        classifier_version: CLASSIFIER_VERSION,
    }
}

/// Read TriggerContext from the GitHub Actions environment + payload JSON.
pub fn from_env() -> TriggerContext {
    let event_name = std::env::var("GITHUB_EVENT_NAME").unwrap_or_default();
    let actor = std::env::var("GITHUB_ACTOR").unwrap_or_default();
    let repo = std::env::var("GITHUB_REPOSITORY").unwrap_or_default();
    let payload = parse_event_payload();

    let head_repo_full_name = payload
        .as_ref()
        .and_then(|p| p.pointer("/pull_request/head/repo/full_name"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let base_repo_full_name = payload
        .as_ref()
        .and_then(|p| p.pointer("/pull_request/base/repo/full_name"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let pr_head_repo_fork = payload
        .as_ref()
        .and_then(|p| p.pointer("/pull_request/head/repo/fork"))
        .and_then(serde_json::Value::as_bool);
    let comment_author_assoc = payload
        .as_ref()
        .and_then(|p| p.pointer("/comment/author_association"))
        .and_then(|v| v.as_str())
        .map(AuthorAssociation::parse)
        .or_else(|| {
            payload
                .as_ref()
                .and_then(|p| p.pointer("/pull_request/author_association"))
                .and_then(|v| v.as_str())
                .map(AuthorAssociation::parse)
        });

    TriggerContext {
        event_name: event_name.clone(),
        actor,
        repo,
        head_repo_full_name,
        base_repo_full_name,
        comment_author_assoc,
        pr_head_repo_fork,
        is_workflow_run_chain: event_name == "workflow_run",
    }
}

fn parse_event_payload() -> Option<serde_json::Value> {
    let path = std::env::var("GITHUB_EVENT_PATH").ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn cache_path() -> Option<PathBuf> {
    std::env::var_os("RUNNER_TEMP").map(|t| PathBuf::from(t).join(VERDICT_CACHE_FILE))
}

/// Return cached verdict if present and version-compatible; otherwise compute and cache.
pub fn cached_or_compute() -> TrustVerdict {
    if let Some(p) = cache_path() {
        if p.exists() {
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<TrustVerdict>(&s) {
                    if v.classifier_version == CLASSIFIER_VERSION {
                        return v;
                    }
                }
            }
        }
    }
    let ctx = from_env();
    let v = classify(&ctx);
    if let Some(p) = cache_path() {
        let _ = std::fs::write(p, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(event: &str) -> TriggerContext {
        TriggerContext {
            event_name: event.into(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_event_is_untrusted_unknown() {
        let v = classify(&ctx(""));
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::Unknown);
    }

    #[test]
    fn push_is_trusted() {
        let v = classify(&ctx("push"));
        assert_eq!(v.level, TrustLevel::Trusted);
        assert_eq!(v.reason, TrustReason::Push);
    }

    #[test]
    fn workflow_dispatch_is_trusted() {
        let v = classify(&ctx("workflow_dispatch"));
        assert_eq!(v.level, TrustLevel::Trusted);
        assert_eq!(v.reason, TrustReason::WorkflowDispatch);
    }

    #[test]
    fn schedule_is_trusted() {
        let v = classify(&ctx("schedule"));
        assert_eq!(v.level, TrustLevel::Trusted);
        assert_eq!(v.reason, TrustReason::Schedule);
    }

    #[test]
    fn internal_pr_is_trusted() {
        let mut c = ctx("pull_request");
        c.pr_head_repo_fork = Some(false);
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Trusted);
        assert_eq!(v.reason, TrustReason::InternalPr);
    }

    #[test]
    fn fork_pr_is_untrusted() {
        let mut c = ctx("pull_request");
        c.pr_head_repo_fork = Some(true);
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::ForkPr);
    }

    #[test]
    fn pull_request_with_unknown_fork_status_is_untrusted_unknown() {
        let c = ctx("pull_request");
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::Unknown);
    }

    #[test]
    fn pull_request_target_is_always_untrusted() {
        let c = ctx("pull_request_target");
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::PullRequestTarget);
    }

    #[test]
    fn comment_from_owner_is_trusted() {
        let mut c = ctx("issue_comment");
        c.comment_author_assoc = Some(AuthorAssociation::Owner);
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Trusted);
    }

    #[test]
    fn comment_from_external_is_untrusted_external_comment() {
        let mut c = ctx("issue_comment");
        c.comment_author_assoc = Some(AuthorAssociation::Contributor);
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::ExternalComment);
    }

    #[test]
    fn comment_from_first_timer_is_untrusted() {
        let mut c = ctx("issue_comment");
        c.comment_author_assoc = Some(AuthorAssociation::FirstTimer);
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::ExternalComment);
    }

    #[test]
    fn comment_with_no_author_assoc_is_untrusted_unknown() {
        let c = ctx("issue_comment");
        let v = classify(&c);
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::Unknown);
    }

    #[test]
    fn workflow_run_is_untrusted() {
        let v = classify(&ctx("workflow_run"));
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::WorkflowRunChain);
    }

    #[test]
    fn unknown_event_is_untrusted() {
        let v = classify(&ctx("repository_dispatch"));
        assert_eq!(v.level, TrustLevel::Untrusted);
        assert_eq!(v.reason, TrustReason::Unknown);
    }

    #[test]
    fn author_association_trusted_set() {
        for a in [
            AuthorAssociation::Owner,
            AuthorAssociation::Member,
            AuthorAssociation::Collaborator,
        ] {
            assert!(a.is_trusted());
        }
    }

    #[test]
    fn author_association_untrusted_set() {
        for a in [
            AuthorAssociation::Contributor,
            AuthorAssociation::FirstTimeContributor,
            AuthorAssociation::FirstTimer,
            AuthorAssociation::Mannequin,
            AuthorAssociation::None,
        ] {
            assert!(!a.is_trusted());
        }
    }

    #[test]
    fn author_association_parse_unknown_string_is_none() {
        assert_eq!(
            AuthorAssociation::parse("WHO_KNOWS"),
            AuthorAssociation::None
        );
    }

    #[test]
    fn verdict_round_trip_serde() {
        let v = TrustVerdict {
            level: TrustLevel::Untrusted,
            reason: TrustReason::ForkPr,
            classifier_version: CLASSIFIER_VERSION,
        };
        let s = serde_json::to_string(&v).unwrap();
        let back: TrustVerdict = serde_json::from_str(&s).unwrap();
        assert_eq!(back.level, v.level);
        assert_eq!(back.reason, v.reason);
        assert_eq!(back.classifier_version, v.classifier_version);
    }
}
