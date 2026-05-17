//! # START OF FILE hainet-collab/src/idle.rs
//! Idle Detector — Ported from PPLPWR's IdleDetector.ts

use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{info, debug};

/// Idle detection for triggering compute tasks
pub struct IdleDetector {
    threshold: Duration,
    last_activity: Instant,
    tx: watch::Sender<bool>,
    pub rx: watch::Receiver<bool>,
}

impl IdleDetector {
    pub fn new(threshold_secs: u64) -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            threshold: Duration::from_secs(threshold_secs),
            last_activity: Instant::now(),
            tx,
            rx,
        }
    }

    /// Report user activity (resets idle timer)
    pub fn report_activity(&mut self) {
        self.last_activity = Instant::now();
        if *self.tx.borrow() {
            debug!("User activity detected — leaving idle state");
            let _ = self.tx.send(false);
        }
    }

    /// Check and update idle state
    pub fn check(&self) -> bool {
        let idle = self.last_activity.elapsed() >= self.threshold;
        if idle != *self.tx.borrow() {
            let _ = self.tx.send(idle);
            if idle {
                info!(threshold_secs = self.threshold.as_secs(), "System is now idle");
            }
        }
        idle
    }

    pub fn is_idle(&self) -> bool {
        *self.rx.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_not_idle() {
        let d = IdleDetector::new(60);
        assert!(!d.is_idle());
    }
}
