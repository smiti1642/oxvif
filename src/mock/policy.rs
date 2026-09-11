//! Explicit opt-in for classified unmodeled effects, not device capability data.

use std::collections::BTreeSet;

/// An unmodeled operation that can return an acknowledgment-only mock response.
///
/// These operations refuse by default with `s:Receiver` / `mock:UnmodeledEffect`.
/// Opting in permits a response shape for workflow tests; it does **not** reset a
/// device, terminate a subscription, generate events or prove a hardware effect.
/// Only common XML/Action/body identity checks are applied, not complete field or
/// subscription validation. Other mock operations are still being classified.
///
/// Select individual operations using [`super::MockTransport::with_acknowledgment_only`]
/// or the corresponding HTTP/replay/adapter builder method. There is no global
/// compatibility switch. This configuration is not part of persisted device state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[non_exhaustive]
pub enum AckOnlyOperation {
    /// Device `SetSystemFactoryDefault`; does not reset any state.
    DeviceFactoryDefault,
    /// Events `Unsubscribe`; does not end a subscription or remove queued events.
    EventsUnsubscribe,
    /// Events `SetSynchronizationPoint`; does not enqueue synchronization events.
    /// This is not Media1/Media2's similarly named operation.
    EventsSynchronizationPoint,
}

impl AckOnlyOperation {
    /// The exact full Action URI selected by this variant, never a suffix match.
    pub const fn action(self) -> &'static str {
        match self {
            Self::DeviceFactoryDefault => {
                "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault"
            }
            Self::EventsUnsubscribe => {
                "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest"
            }
            Self::EventsSynchronizationPoint => {
                "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest"
            }
        }
    }

    pub(crate) fn for_action(action: &str) -> Option<Self> {
        [
            Self::DeviceFactoryDefault,
            Self::EventsUnsubscribe,
            Self::EventsSynchronizationPoint,
        ]
        .into_iter()
        .find(|operation| operation.action() == action)
    }
}

#[derive(Clone, Default)]
pub(crate) struct AckOnlyPolicy(BTreeSet<AckOnlyOperation>);

impl AckOnlyPolicy {
    pub(crate) fn enable(&mut self, operation: AckOnlyOperation) {
        self.0.insert(operation);
    }

    pub(crate) fn refusal(&self, action: &str) -> Option<String> {
        let operation = AckOnlyOperation::for_action(action)?;
        if self.0.contains(&operation) {
            return None;
        }
        use super::fault::{Code, Fault, MOCK_UNMODELED_EFFECT};
        Some(Fault::new(Code::Receiver, &[MOCK_UNMODELED_EFFECT],
            "This mock does not model the requested effect; explicitly opt in to acknowledgment-only behavior"
        ).to_xml())
    }
}
