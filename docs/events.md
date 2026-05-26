# Events Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/events/wsdl/event.wsdl
- **Namespace:** `http://www.onvif.org/ver10/events/wsdl` (prefix `tev`); WS-BaseNotification
  types use `wsnt` (`http://docs.oasis-open.org/wsn/b-2`).
- **ONVIF Profile:** S / T
- **oxvif status:** ◐ implemented in `src/client/events.rs` — pull-point + push subscribe + consumer.

ONVIF events span several portTypes: the device `EventPortType`, the per-subscription
`PullPointSubscription` / `SubscriptionManager` (WS-BaseNotification), and the consumer-side
`NotificationConsumer` (`Notify`).

---

## Operations

| Operation | PortType | Purpose | oxvif | method |
|-----------|----------|---------|:----:|--------|
| GetEventProperties | Event | list topics + filter dialects | ✓ | `get_event_properties` |
| CreatePullPointSubscription | Event | create pull-point subscription | ✓ | `create_pull_point_subscription` |
| PullMessages | PullPointSubscription | poll queued events | ✓ | `pull_messages` |
| Renew | SubscriptionManager | extend subscription | ✓ | `renew_subscription` |
| Unsubscribe | SubscriptionManager | cancel subscription | ✓ | `unsubscribe` |
| SetSynchronizationPoint | PullPointSubscription | force property sync | ✓ | `set_synchronization_point` |
| Subscribe | NotificationProducer | push (base-notification) subscribe | ✓ | `subscribe` |
| Notify (consumer) | NotificationConsumer | receive pushed `Notify` | ✓ | `notification_listener` |
| _(poll loop helper)_ | — | continuous stream | ✓ | `event_stream` |
| GetServiceCapabilities | Event | event service capabilities | — | — |
| Seek | PullPointSubscription | replay events from a timestamp | — | — |
| GetCurrentMessage | NotificationProducer | latest message for a topic | — | — |
| PauseSubscription | PausableSubscriptionManager | pause delivery | — | — |
| ResumeSubscription | PausableSubscriptionManager | resume delivery | — | — |
| AddEventBroker | Event | configure MQTT/event broker | — | — |
| DeleteEventBroker | Event | remove an event broker | — | — |
| GetEventBrokers | Event | list event brokers | — | — |

(`CreatePullPoint` / `DestroyPullPoint` / `GetMessages` from the raw `wsnt` PullPoint portType are
not used by ONVIF devices in practice — the ONVIF pull-point flow above is the relevant one.)

---

## Request / response detail (unimplemented)

#### GetServiceCapabilities
- **Req:** _(empty)_ · **Resp:** `Capabilities` `tev:Capabilities` [1]
  (attrs incl. `WSSubscriptionPolicySupport`, `WSPullPointSupport`, `MaxNotificationProducers`,
  `MaxPullPoints`, `PersistentNotificationStorage`, `EventBrokerProtocols`).

#### Seek (Profile G event replay)
- **Req:** `UtcTime` `xs:dateTime` [1]; `Reverse` `xs:boolean` [0..1] · **Resp:** _(empty)_

#### PauseSubscription / ResumeSubscription
- **Req:** _(empty body; addressed to the subscription manager EPR)_ · **Resp:** _(empty)_

#### GetCurrentMessage
- **Req:** `Topic` (`wsnt:TopicExpression`) [1] · **Resp:** current `Message` payload.

#### AddEventBroker
- **Req:** `EventBroker` `tev:EventBrokerConfig` [1] (Address, TopicPrefix, User, Password,
  CertificateID, PublishFilter, QoS, …) · **Resp:** _(empty)_

#### DeleteEventBroker
- **Req:** `Address` `xs:anyURI` [1] · **Resp:** _(empty)_

#### GetEventBrokers
- **Req:** `Address` `xs:anyURI` [0..1] · **Resp:** `EventBroker` `tev:EventBrokerConfig` [0..*]

Complex types (`tev:Capabilities`, `tev:EventBrokerConfig`, `wsnt:TopicExpression`,
`wsnt:NotificationMessageHolderType`): see event.wsdl / b-2.xsd.

_Source: event.wsdl operation list (fetched 2026-05); broker/seek field shapes are standard ONVIF —
verify against event.wsdl when implementing._
