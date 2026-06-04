use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct TokenBucket {
    capacity: u64,
    refill_rate: f64,
    tokens: AtomicU64,
    last_refill: AtomicU64,
}

impl TokenBucket {
    pub const fn new(capacity: u64, rate_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_rate: rate_per_sec,
            tokens: AtomicU64::new(capacity),
            last_refill: AtomicU64::new(0),
        }
    }

    pub fn try_consume(&self, n: u64) -> bool {
        let now = now_nanos();
        let last = self.last_refill.load(Ordering::Acquire);
        let elapsed_ns = now.saturating_sub(last) as f64;
        let new_tokens = (elapsed_ns / 1_000_000_000.0 * self.refill_rate) as u64;

        if new_tokens > 0
            && self
                .last_refill
                .compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            let mut current = self.tokens.load(Ordering::Acquire);
            loop {
                let refilled = (current + new_tokens).min(self.capacity);
                match self.tokens.compare_exchange(
                    current,
                    refilled,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(actual) => current = actual,
                }
            }
        }

        let mut current = self.tokens.load(Ordering::Acquire);
        loop {
            if current < n {
                return false;
            }
            match self.tokens.compare_exchange(
                current,
                current - n,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

fn now_nanos() -> u64 {
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_nanos() as u64
}

pub struct RateLimiters {
    pub did_open: TokenBucket,
    pub did_change: TokenBucket,
    pub did_save: TokenBucket,
    pub completion: TokenBucket,
    pub execute_command: TokenBucket,
    pub apply_edit: TokenBucket,
    pub hover: TokenBucket,
    pub code_lens: TokenBucket,
    pub code_action: TokenBucket,
    pub references: TokenBucket,
    pub rename: TokenBucket,
    pub inlay_hint: TokenBucket,
    pub folding_range: TokenBucket,
    pub document_symbol: TokenBucket,
    pub semantic_tokens: TokenBucket,
    pub goto_definition: TokenBucket,
    pub formatting: TokenBucket,
    pub exposure_map: TokenBucket,
}

impl Default for RateLimiters {
    fn default() -> Self {
        Self {
            did_open: TokenBucket::new(10, 10.0),
            did_change: TokenBucket::new(100, 100.0),
            did_save: TokenBucket::new(5, 5.0),
            completion: TokenBucket::new(50, 50.0),
            execute_command: TokenBucket::new(20, 20.0),
            apply_edit: TokenBucket::new(10, 10.0),
            hover: TokenBucket::new(50, 50.0),
            code_lens: TokenBucket::new(10, 10.0),
            code_action: TokenBucket::new(20, 20.0),
            references: TokenBucket::new(10, 10.0),
            rename: TokenBucket::new(5, 5.0),
            inlay_hint: TokenBucket::new(50, 50.0),
            folding_range: TokenBucket::new(10, 10.0),
            document_symbol: TokenBucket::new(10, 10.0),
            semantic_tokens: TokenBucket::new(50, 50.0),
            goto_definition: TokenBucket::new(20, 20.0),
            formatting: TokenBucket::new(10, 10.0),
            exposure_map: TokenBucket::new(5, 0.083),
        }
    }
}

pub fn timing_jitter_micros() -> u64 {
    let nanos = now_nanos();
    5_000 + (nanos % 10_000)
}
