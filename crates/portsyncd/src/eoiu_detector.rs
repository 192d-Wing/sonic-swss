//! EOIU (End of Init sequence User indication) signal detection
//!
//! The Linux kernel sends a special netlink message at the end of initial port state
//! synchronization. This module detects that signal to coordinate warm restart transitions.
//!
//! ## EOIU Signal Detection
//!
//! The EOIU signal is identified by a netlink RTM_NEWLINK message where:
//! - Interface name is a special marker (traditionally "lo" loopback with flags=0)
//! - OR the `ifi_change` field is 0 (indicates end of dump)
//! - The message has no actual attribute changes
//!
//! When this is detected during warm restart warm restart initial sync, we know
//! the kernel has finished sending all initial port state and it's safe to accept
//! APP_DB updates again.
//!
//! NIST 800-53 SC-24: Fail-secure - if EOIU detection fails, keep initial sync locked
//!
//! Phase 6 Week 2 implementation.

/// EOIU detection state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EoiuDetectionState {
    /// Waiting for EOIU signal (normal startup or warm restart initial sync)
    Waiting,
    /// EOIU signal detected
    Detected,
    /// EOIU already processed, ignore further signals
    Complete,
}

/// EOIU detector - identifies when kernel initial state sync is complete
#[derive(Debug)]
pub struct EoiuDetector {
    state: EoiuDetectionState,
    messages_seen: u32,
    dumped_interfaces: u32,
}

impl EoiuDetector {
    /// Create new EOIU detector
    pub fn new() -> Self {
        Self {
            state: EoiuDetectionState::Waiting,
            messages_seen: 0,
            dumped_interfaces: 0,
        }
    }

    /// Get current detection state
    pub fn state(&self) -> EoiuDetectionState {
        self.state
    }

    /// Check if EOIU signal has been detected
    pub fn is_detected(&self) -> bool {
        self.state == EoiuDetectionState::Detected
    }

    /// Process a netlink RTM_NEWLINK message to detect EOIU
    ///
    /// # Arguments
    /// * `interface_name` - Name of the interface from the netlink message
    /// * `ifi_change` - Change mask from netlink message (0 = EOIU indicator)
    /// * `ifi_flags` - Flags field (IFF_UP, IFF_RUNNING, etc.)
    ///
    /// # Returns
    /// true if this message indicates EOIU, false otherwise
    pub fn check_eoiu(&mut self, interface_name: &str, ifi_change: u32, _ifi_flags: u32) -> bool {
        self.messages_seen += 1;

        if self.state == EoiuDetectionState::Complete {
            // Already processed EOIU, don't detect again
            return false;
        }

        // EOIU indicators:
        // 1. ifi_change == 0 means "no actual change" - used to mark end of dump
        // 2. Special marker: loopback interface with ifi_change == 0
        // 3. Or simply: any NEWLINK with ifi_change == 0 signals end of initial dump

        let is_eoiu = ifi_change == 0;

        if is_eoiu && self.state == EoiuDetectionState::Waiting {
            self.state = EoiuDetectionState::Detected;
            eprintln!(
                "portsyncd: EOIU signal detected on interface '{}' (messages_seen={})",
                interface_name, self.messages_seen
            );
            return true;
        }

        false
    }

    /// Mark EOIU as processed (transition to Complete state)
    pub fn mark_complete(&mut self) {
        if self.state == EoiuDetectionState::Detected {
            self.state = EoiuDetectionState::Complete;
            eprintln!(
                "portsyncd: EOIU processing complete (total messages: {})",
                self.messages_seen
            );
        }
    }

    /// Reset detector for testing or manual reset
    pub fn reset(&mut self) {
        self.state = EoiuDetectionState::Waiting;
        self.messages_seen = 0;
        self.dumped_interfaces = 0;
    }

    /// Increment dumped interfaces counter
    pub fn increment_dumped_interfaces(&mut self) {
        if self.state == EoiuDetectionState::Waiting {
            self.dumped_interfaces += 1;
        }
    }

    /// Get number of interfaces dumped before EOIU
    pub fn dumped_interfaces(&self) -> u32 {
        self.dumped_interfaces
    }

    /// Get number of messages seen
    pub fn messages_seen(&self) -> u32 {
        self.messages_seen
    }
}

impl Default for EoiuDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eoiu_detector_creation() {
        let detector = EoiuDetector::new();
        assert_eq!(detector.state(), EoiuDetectionState::Waiting);
        assert!(!detector.is_detected());
        assert_eq!(detector.messages_seen(), 0);
    }

    #[test]
    fn test_eoiu_detector_waiting_state() {
        let mut detector = EoiuDetector::new();

        // Normal interface update (ifi_change != 0)
        let is_eoiu = detector.check_eoiu("Ethernet0", 1, 0x41);
        assert!(!is_eoiu);
        assert_eq!(detector.state(), EoiuDetectionState::Waiting);
        assert_eq!(detector.messages_seen(), 1);
    }

    #[test]
    fn test_eoiu_detector_ifi_change_zero() {
        let mut detector = EoiuDetector::new();

        // Update with ifi_change = 0 (EOIU marker)
        let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
        assert!(is_eoiu);
        assert_eq!(detector.state(), EoiuDetectionState::Detected);
        assert_eq!(detector.messages_seen(), 1);
    }

    #[test]
    fn test_eoiu_detector_sequence() {
        let mut detector = EoiuDetector::new();

        // Simulate initial port dump
        assert!(!detector.check_eoiu("Ethernet0", 1, 0x41));
        detector.increment_dumped_interfaces();

        assert!(!detector.check_eoiu("Ethernet4", 1, 0x41));
        detector.increment_dumped_interfaces();

        assert!(!detector.check_eoiu("Ethernet8", 1, 0x41));
        detector.increment_dumped_interfaces();

        // EOIU signal arrives
        assert!(detector.check_eoiu("lo", 0, 0x01));
        assert_eq!(detector.dumped_interfaces(), 3);
        assert_eq!(detector.messages_seen(), 4);
    }

    #[test]
    fn test_eoiu_detector_ignore_after_detection() {
        let mut detector = EoiuDetector::new();

        // Detect EOIU
        detector.check_eoiu("lo", 0, 0x01);
        assert!(detector.is_detected());

        // Mark as complete
        detector.mark_complete();
        assert_eq!(detector.state(), EoiuDetectionState::Complete);

        // Subsequent EOIU-like messages should be ignored
        let is_eoiu = detector.check_eoiu("lo", 0, 0x01);
        assert!(!is_eoiu);
    }

    #[test]
    fn test_eoiu_detector_reset() {
        let mut detector = EoiuDetector::new();

        detector.check_eoiu("lo", 0, 0x01);
        assert!(detector.is_detected());

        detector.reset();
        assert_eq!(detector.state(), EoiuDetectionState::Waiting);
        assert_eq!(detector.messages_seen(), 0);
        assert_eq!(detector.dumped_interfaces(), 0);
    }

    #[test]
    fn test_eoiu_detector_multiple_interfaces() {
        let mut detector = EoiuDetector::new();

        // Simulate multiple interface updates
        for i in 0..10 {
            let ifname = format!("Ethernet{}", i * 4);
            assert!(!detector.check_eoiu(&ifname, 1, 0x41));
            detector.increment_dumped_interfaces();
        }

        assert_eq!(detector.dumped_interfaces(), 10);
        assert_eq!(detector.messages_seen(), 10);

        // EOIU arrives
        assert!(detector.check_eoiu("lo", 0, 0x01));
        assert!(detector.is_detected());
        assert_eq!(detector.messages_seen(), 11);
    }

    #[test]
    fn test_eoiu_detector_default() {
        let detector = EoiuDetector::default();
        assert_eq!(detector.state(), EoiuDetectionState::Waiting);
        assert!(!detector.is_detected());
    }
}
