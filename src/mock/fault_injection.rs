//! Test-only fault injection for the mock server.
//!
//! C++ / Rust tests POST to `/admin/inject_fault?action=...&code=...&reason=...`
//! to arm a single-shot SOAP Fault keyed by action suffix. The next
//! matching ONVIF call consumes the entry and the mock returns that
//! Fault instead of the canned success response. `POST /admin/clear_faults`
//! drops all pending entries (e.g. between test fixtures).
//!
//! Faults are intentionally *not* persisted; this is ephemeral test
//! state that lives alongside but separate from the device's TOML state.

use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct PendingFault {
    /// Matched against `action.ends_with(&action_suffix)`. Typically a
    /// bare operation name like `"GetProfiles"`, but a full URI tail
    /// (e.g. `"ver10/media/wsdl/GetProfiles"`) also works for
    /// disambiguation if the same op exists in multiple namespaces.
    pub action_suffix: String,
    /// SOAP 1.2 fault code, e.g. `"ter:NotAuthorized"` /
    /// `"ter:InvalidArgs"` / `"s:Receiver"`.
    pub code: String,
    /// Human-readable reason text. Surfaces in the consumer side as
    /// part of the SOAP fault Reason/Text element.
    pub reason: String,
}

pub struct FaultInjector {
    pending: Mutex<Vec<PendingFault>>,
}

impl Default for FaultInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Queue a fault. Multiple may be queued; they consume in insertion
    /// order on matching actions.
    pub fn inject(&self, fault: PendingFault) {
        self.pending.lock().unwrap().push(fault);
    }

    /// Return and remove the first pending fault whose `action_suffix`
    /// matches the tail of `action`. Single-shot — once consumed the
    /// entry is gone. Returns `None` if no match.
    pub fn take_for_action(&self, action: &str) -> Option<PendingFault> {
        let mut pending = self.pending.lock().unwrap();
        let idx = pending
            .iter()
            .position(|f| action.ends_with(&f.action_suffix))?;
        Some(pending.remove(idx))
    }

    /// Drop every queued fault. Typical use: between test fixtures so
    /// stray injections from a previous case do not leak.
    pub fn clear_all(&self) {
        self.pending.lock().unwrap().clear();
    }

    /// Snapshot of currently queued faults (test introspection /
    /// debugging only — not used by the dispatch path).
    #[allow(dead_code)]
    pub fn pending_snapshot(&self) -> Vec<PendingFault> {
        self.pending.lock().unwrap().clone()
    }
}
