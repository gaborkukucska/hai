// START OF FILE hainet-social/src/congestion.rs
//! AIMD Congestion Control
//! 
//! Implements Additive Increase Multiplicative Decrease for media chunk transmission
//! over the mesh network to prevent network flooding and handle backpressure.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Default starting window size (number of concurrent chunks in flight)
const INITIAL_WINDOW: u32 = 4;
/// Maximum allowed window size
const MAX_WINDOW: u32 = 128;
/// Multiplicative decrease factor (e.g. 0.5 means cut in half)
const DECREASE_FACTOR: f32 = 0.5;
/// How long before a packet is considered lost (timeout)
const TIMEOUT_MS: u64 = 5000;

/// Manages congestion control state for a specific peer or download
#[derive(Debug)]
pub struct CongestionController {
    /// Current window size
    window_size: AtomicU32,
    /// Number of packets currently in flight
    in_flight: AtomicU32,
    /// Slow start threshold
    ssthresh: AtomicU32,
    /// Last packet send time
    last_send_time: std::sync::RwLock<Instant>,
    /// Estimated RTT in milliseconds
    rtt_ms: AtomicU64,
}

impl CongestionController {
    pub fn new() -> Self {
        Self {
            window_size: AtomicU32::new(INITIAL_WINDOW),
            in_flight: AtomicU32::new(0),
            ssthresh: AtomicU32::new(MAX_WINDOW),
            last_send_time: std::sync::RwLock::new(Instant::now()),
            rtt_ms: AtomicU64::new(500),
        }
    }

    /// Check if we are allowed to send another chunk
    pub fn can_send(&self) -> bool {
        self.in_flight.load(Ordering::Relaxed) < self.window_size.load(Ordering::Relaxed)
    }

    /// Record that a chunk has been sent
    pub fn on_send(&self) {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let mut last_send = self.last_send_time.write().unwrap();
        *last_send = Instant::now();
    }

    /// Record that an ACK was received (successful transmission)
    pub fn on_ack(&self, rtt: Duration) {
        // Decrease in-flight counter
        let in_flight = self.in_flight.load(Ordering::Relaxed);
        if in_flight > 0 {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }

        // Update RTT estimate (Exponential Moving Average)
        let current_rtt = self.rtt_ms.load(Ordering::Relaxed);
        let new_rtt = (current_rtt * 7 + rtt.as_millis() as u64) / 8;
        self.rtt_ms.store(new_rtt, Ordering::Relaxed);

        let window = self.window_size.load(Ordering::Relaxed);
        let ssthresh = self.ssthresh.load(Ordering::Relaxed);

        if window < ssthresh {
            // Slow start phase: double window size every RTT
            // For each ACK, increase by 1
            let new_window = std::cmp::min(window + 1, MAX_WINDOW);
            self.window_size.store(new_window, Ordering::SeqCst);
            debug!("Slow start: Window increased to {}", new_window);
        } else {
            // Congestion avoidance phase (AIMD)
            // Increase window by 1/window for each ACK (additive increase of 1 per RTT)
            // Note: Since we are using integers, this is a simplified approximation
            let new_window = std::cmp::min(window + 1, MAX_WINDOW);
            self.window_size.store(new_window, Ordering::SeqCst);
            debug!("Congestion avoidance: Window increased to {}", new_window);
        }
    }

    /// Record a timeout or packet loss (congestion detected)
    pub fn on_loss(&self) {
        let window = self.window_size.load(Ordering::Relaxed);
        
        // Multiplicative decrease
        let new_ssthresh = std::cmp::max(2, (window as f32 * DECREASE_FACTOR) as u32);
        self.ssthresh.store(new_ssthresh, Ordering::SeqCst);
        
        // Reset window to 1 (Reno style) or to ssthresh (Fast Recovery style)
        // We'll use a conservative approach: drop to 1
        self.window_size.store(1, Ordering::SeqCst);
        
        warn!("Packet loss detected. Window decreased from {} to 1, ssthresh: {}", window, new_ssthresh);
    }
    
    /// Get current metrics
    pub fn metrics(&self) -> (u32, u32, u64) {
        (
            self.window_size.load(Ordering::Relaxed),
            self.in_flight.load(Ordering::Relaxed),
            self.rtt_ms.load(Ordering::Relaxed)
        )
    }
}

impl Default for CongestionController {
    fn default() -> Self {
        Self::new()
    }
}
