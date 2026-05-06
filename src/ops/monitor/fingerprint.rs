//! AI Tool Fingerprinter: behavioral fingerprinting, identity verification, and trust management.
//!
//! Consumes [`MonitorEvent`]s to generate per-tool behavioral fingerprints,
//! verifies tool identity claims, and maintains trust scores.
//! All stores use [`DashMap`] wrapped in [`Arc`] for lock-free concurrent access.

use crate::ops::monitor::*;
use chrono::Utc;
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::Arc;

// ─── Helpers ─────────────────────────────────────────────────────────────────

#[inline]
fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[inline]
fn hash_features(features: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(features.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

fn extract_features(events: &[MonitorEvent]) -> String {
    let access_frequency = events.len();

    let unique_keys: HashSet<_> = events.iter().map(|e| &e.secret_key).collect();
    let key_diversity = unique_keys.len();

    // Temporal pattern: standard deviation of inter-event intervals (seconds).
    let temporal_pattern = if events.len() >= 2 {
        let timestamps: Vec<i64> = events.iter().map(|e| e.timestamp.timestamp()).collect();
        let diffs: Vec<f64> = timestamps
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64)
            .collect();
        if diffs.is_empty() {
            "0".to_string()
        } else {
            let mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
            let variance =
                diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / diffs.len() as f64;
            format!("{:.6}", variance.sqrt())
        }
    } else {
        "0".to_string()
    };

    // Operation distribution: count per operation type.
    let mut op_counts = std::collections::BTreeMap::new();
    for event in events {
        *op_counts.entry(event.operation.clone()).or_insert(0usize) += 1;
    }
    let operation_distribution = op_counts
        .into_iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "af:{}|kd:{}|tp:{}|od:{}",
        access_frequency, key_diversity, temporal_pattern, operation_distribution
    )
}

// ─── Fingerprint Generator ───────────────────────────────────────────────────

/// Generates and stores behavioral fingerprints for AI tools.
///
/// Backed by an [`Arc<DashMap>`] so it can be cloned cheaply and shared
/// across threads.
#[derive(Debug, Clone)]
pub struct FingerprintGenerator {
    store: Arc<DashMap<String, ToolFingerprint>>,
}

impl Default for FingerprintGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FingerprintGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
        }
    }

    /// Generate a fingerprint from events **and persist** it.
    pub fn generate_and_store(
        &self,
        tool_type: &ToolType,
        events: &[MonitorEvent],
    ) -> Result<ToolFingerprint, MonitorError> {
        let fp = self.generate_internal(tool_type, events)?;
        self.store.insert(tool_type.to_string(), fp.clone());
        Ok(fp)
    }

    /// Generate a temporary fingerprint **without persisting**.
    pub fn generate_temporary(
        &self,
        tool_type: &ToolType,
        events: &[MonitorEvent],
    ) -> Result<ToolFingerprint, MonitorError> {
        self.generate_internal(tool_type, events)
    }

    /// Retrieve a stored fingerprint.
    pub fn get_fingerprint(&self, tool_type: &ToolType) -> Option<ToolFingerprint> {
        self.store.get(&tool_type.to_string()).map(|e| e.clone())
    }

    pub(crate) fn store(&self) -> &Arc<DashMap<String, ToolFingerprint>> {
        &self.store
    }

    fn generate_internal(
        &self,
        tool_type: &ToolType,
        events: &[MonitorEvent],
    ) -> Result<ToolFingerprint, MonitorError> {
        if events.is_empty() {
            return Err(MonitorError::InsufficientEvents(0, 1));
        }

        let features = extract_features(events);
        let signature = hash_features(&features);
        let confidence = (events.len() as f64 / 100.0).min(1.0);

        Ok(ToolFingerprint {
            tool_type: tool_type.clone(),
            behavioral_signature: signature,
            created_at: Utc::now(),
            confidence,
        })
    }
}

// ─── Identity Verifier ───────────────────────────────────────────────────────

/// Verifies AI tool identity claims against stored fingerprints.
#[derive(Debug, Clone)]
pub struct IdentityVerifier {
    generator: FingerprintGenerator,
}

impl IdentityVerifier {
    #[must_use]
    pub fn new(generator: FingerprintGenerator) -> Self {
        Self { generator }
    }

    /// Verify a claimed tool identity.
    ///
    /// Returns [`VerificationResult::InsufficientData`] when fewer than 30
    /// events are provided, and [`VerificationResult::NoBaseline`] when no
    /// stored fingerprint exists for the claimed tool.
    pub fn verify(
        &self,
        claimed_tool: &ToolType,
        events: &[MonitorEvent],
    ) -> Result<VerificationResult, MonitorError> {
        if events.len() < 30 {
            return Ok(VerificationResult::InsufficientData);
        }

        let stored = match self.generator.get_fingerprint(claimed_tool) {
            Some(fp) => fp,
            None => return Ok(VerificationResult::NoBaseline),
        };

        let temp_fp = self.generator.generate_temporary(claimed_tool, events)?;
        let claimed_matches = temp_fp.behavioral_signature == stored.behavioral_signature;

        if claimed_matches {
            return Ok(VerificationResult::Match);
        }

        // Check for impersonation: temp signature matches a *different* tool?
        let mut matched_tool: Option<String> = None;
        for entry in self.generator.store().iter() {
            let (tool, fp) = entry.pair();
            if tool != &claimed_tool.to_string()
                && fp.behavioral_signature == temp_fp.behavioral_signature
            {
                matched_tool = Some(tool.clone());
                break;
            }
        }

        let confidence = if matched_tool.is_some() { 0.9 } else { 0.5 };
        let divergence = if let Some(ref t) = matched_tool {
            format!(
                "impersonation suspected: behavior matches '{}' instead of claimed '{}'",
                t, claimed_tool
            )
        } else {
            format!("behavioral_signature mismatch for '{}'", claimed_tool)
        };

        Ok(VerificationResult::Mismatch {
            confidence,
            divergence,
        })
    }
}

// ─── Trust Manager ───────────────────────────────────────────────────────────

/// Manages trust scores for AI tools via event-driven updates.
#[derive(Debug, Clone)]
pub struct TrustManager {
    scores: Arc<DashMap<String, TrustScore>>,
    config: TrustConfig,
}

impl TrustManager {
    #[must_use]
    pub fn new(config: TrustConfig) -> Self {
        Self {
            scores: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Update trust score based on a trust event.
    pub fn update_trust(
        &self,
        tool_type: &ToolType,
        event: TrustEvent,
    ) -> Result<TrustScore, MonitorError> {
        let key = tool_type.to_string();
        let now = Utc::now();

        let mut score = self.scores.entry(key).or_insert(TrustScore {
            score: 0.5,
            confidence: 0.0,
            last_updated: now,
            sample_size: 0,
        });

        let adjustment = match event {
            TrustEvent::PositiveVerification => self.config.positive_weight,
            TrustEvent::NegativeVerification => self.config.negative_weight,
            TrustEvent::SuspiciousBehavior => self.config.suspicious_weight,
            TrustEvent::NormalBehavior => self.config.normal_weight,
        };

        score.score = (score.score + adjustment).clamp(0.0, 1.0);
        score.sample_size += 1;
        score.confidence = (score.sample_size as f64 / 100.0).min(1.0);
        score.last_updated = now;

        Ok(*score)
    }

    /// Retrieve trust score for a tool.
    pub fn get_trust_score(&self, tool_type: &ToolType) -> Option<TrustScore> {
        self.scores.get(&tool_type.to_string()).map(|e| *e.value())
    }

    /// Retrieve all trust scores.
    pub fn get_all_scores(&self) -> Vec<(String, TrustScore)> {
        self.scores
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect()
    }

    /// Reset a tool's trust score to neutral.
    pub fn reset_score(&self, tool_type: &ToolType) {
        let now = Utc::now();
        self.scores.insert(
            tool_type.to_string(),
            TrustScore {
                score: 0.5,
                confidence: 0.0,
                last_updated: now,
                sample_size: 0,
            },
        );
    }

    /// Apply time-based decay to a tool's trust score.
    pub fn apply_decay(&self, tool_type: &ToolType) -> Result<TrustScore, MonitorError> {
        let key = tool_type.to_string();
        let mut score = self
            .scores
            .get_mut(&key)
            .ok_or_else(|| MonitorError::TrustScoreNotFound(key.clone()))?;

        let now = Utc::now();
        let hours_since_update = (now - score.last_updated).num_hours() as f64;
        let days = hours_since_update / 24.0;

        if days > 0.0 {
            let decay = days * self.config.decay_rate;
            score.score = (score.score - decay).max(0.0);
            score.last_updated = now;
        }

        Ok(*score)
    }
}

// ─── System Builder ──────────────────────────────────────────────────────────

/// Bundles generator, verifier, and trust manager for easy setup.
///
/// All components are [`Clone`] and thread-safe via [`Arc`].
#[derive(Debug, Clone)]
pub struct FingerprinterSystem {
    pub generator: FingerprintGenerator,
    pub verifier: IdentityVerifier,
    pub trust_manager: TrustManager,
}

impl Default for FingerprinterSystem {
    fn default() -> Self {
        Self::new(TrustConfig::default())
    }
}

impl FingerprinterSystem {
    #[must_use]
    pub fn new(config: TrustConfig) -> Self {
        let generator = FingerprintGenerator::new();
        let verifier = IdentityVerifier::new(generator.clone());
        let trust_manager = TrustManager::new(config);
        Self {
            generator,
            verifier,
            trust_manager,
        }
    }
}
