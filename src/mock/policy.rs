//! Explicit opt-in for classified unmodeled effects, not device capability data.

use std::collections::BTreeSet;

/// An unmodeled operation that can return an acknowledgment-only mock response.
///
/// These operations refuse by default with `s:Receiver` / `mock:UnmodeledEffect`.
/// Opting in permits a response shape for workflow tests; it does **not** reset a
/// device, execute auxiliary commands, reboot/upgrade/restore, manage subscription
/// or search lifetimes, generate events or prove a hardware effect. Upload URIs,
/// subscription references and timestamps in these responses are fixture data.
/// Most selections only apply common XML/Action/body identity checks, not full
/// field or subscription validation. Media synchronization additionally validates
/// its scoped profile selector. Other mock operations are still being classified.
///
/// Select individual operations using [`super::MockTransport::with_acknowledgment_only`]
/// or the corresponding HTTP/replay/adapter builder method. There is no global
/// compatibility switch. This configuration is not part of persisted device state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[non_exhaustive]
pub enum AckOnlyOperation {
    /// Media1 synchronization receipt; does not emit video or metadata streams.
    MediaSynchronizationPoint,
    /// Media2 synchronization receipt; independent of Media1 and Events policy.
    Media2SynchronizationPoint,
    /// Device `SetSystemFactoryDefault`; does not reset any state.
    DeviceFactoryDefault,
    /// Events `Unsubscribe`; does not end a subscription or remove queued events.
    EventsUnsubscribe,
    /// Events `SetSynchronizationPoint`; does not enqueue synchronization events.
    /// This is not Media1/Media2's similarly named operation.
    EventsSynchronizationPoint,
    /// Device `SendAuxiliaryCommand`; does not execute the command.
    DeviceAuxiliaryCommand,
    /// PTZ `SendAuxiliaryCommand`; retains the legacy command allowlist but does
    /// not validate profile identity or execute the command.
    PtzAuxiliaryCommand,
    /// Device `SystemReboot`; returns an explicit no-reboot receipt.
    DeviceReboot,
    /// Device `StartFirmwareUpgrade`; the upload URI is a fixture, not an upload service.
    DeviceFirmwareUpgrade,
    /// Device `StartSystemRestore`; does not accept uploads or restore state.
    DeviceSystemRestore,
    /// Events `Subscribe`; does not create a push subscription or send notifications.
    EventsSubscribe,
    /// Events `Renew`; does not extend subscription lifetime.
    EventsRenew,
    /// Search `EndSearch`; does not terminate or expire a search session.
    SearchEnd,
}

impl AckOnlyOperation {
    /// The exact full Action URI selected by this variant, never a suffix match.
    pub const fn action(self) -> &'static str {
        match self {
            Self::MediaSynchronizationPoint => {
                "http://www.onvif.org/ver10/media/wsdl/SetSynchronizationPoint"
            }
            Self::Media2SynchronizationPoint => {
                "http://www.onvif.org/ver20/media/wsdl/SetSynchronizationPoint"
            }
            Self::DeviceFactoryDefault => {
                "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault"
            }
            Self::EventsUnsubscribe => {
                "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/UnsubscribeRequest"
            }
            Self::EventsSynchronizationPoint => {
                "http://www.onvif.org/ver10/events/wsdl/PullPointSubscription/SetSynchronizationPointRequest"
            }
            Self::DeviceAuxiliaryCommand => {
                "http://www.onvif.org/ver10/device/wsdl/SendAuxiliaryCommand"
            }
            Self::PtzAuxiliaryCommand => "http://www.onvif.org/ver20/ptz/wsdl/SendAuxiliaryCommand",
            Self::DeviceReboot => "http://www.onvif.org/ver10/device/wsdl/SystemReboot",
            Self::DeviceFirmwareUpgrade => {
                "http://www.onvif.org/ver10/device/wsdl/StartFirmwareUpgrade"
            }
            Self::DeviceSystemRestore => {
                "http://www.onvif.org/ver10/device/wsdl/StartSystemRestore"
            }
            Self::EventsSubscribe => {
                "http://docs.oasis-open.org/wsn/bw-2/NotificationProducer/SubscribeRequest"
            }
            Self::EventsRenew => {
                "http://docs.oasis-open.org/wsn/bw-2/SubscriptionManager/RenewRequest"
            }
            Self::SearchEnd => "http://www.onvif.org/ver10/search/wsdl/EndSearch",
        }
    }

    pub(crate) fn for_action(action: &str) -> Option<Self> {
        [
            Self::MediaSynchronizationPoint,
            Self::Media2SynchronizationPoint,
            Self::DeviceFactoryDefault,
            Self::EventsUnsubscribe,
            Self::EventsSynchronizationPoint,
            Self::DeviceAuxiliaryCommand,
            Self::PtzAuxiliaryCommand,
            Self::DeviceReboot,
            Self::DeviceFirmwareUpgrade,
            Self::DeviceSystemRestore,
            Self::EventsSubscribe,
            Self::EventsRenew,
            Self::SearchEnd,
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
