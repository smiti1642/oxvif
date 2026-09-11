//! This external crate preserves old struct-literal and serde consumers.
use oxvif::NotificationMessage;

fn legacy_literal() -> NotificationMessage {
    NotificationMessage {
        topic: "topic".into(),
        utc_time: "time".into(),
        property_operation: "Changed".into(),
        source: Default::default(),
        data: Default::default(),
    }
}

#[test]
fn legacy_notification_literal_and_listener_signature_remain_compatible() {
    assert_eq!(legacy_literal().topic, "topic");
    fn accepts_old_signature(
        _: fn(
            std::net::SocketAddr,
        )
            -> std::pin::Pin<Box<dyn futures_core::Stream<Item = NotificationMessage> + Send>>,
    ) {
    }
    accepts_old_signature(oxvif::notification_listener);
}

#[cfg(feature = "serde")]
#[test]
fn legacy_notification_json_has_no_transport_fields() {
    let expected = serde_json::json!({"topic":"topic", "utc_time":"time", "property_operation":"Changed", "source":{}, "data":{}});
    assert_eq!(serde_json::to_value(legacy_literal()).unwrap(), expected);
    let decoded: NotificationMessage = serde_json::from_value(expected).unwrap();
    assert_eq!(decoded.property_operation, "Changed");
}
