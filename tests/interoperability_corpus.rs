#![cfg(feature = "metamorph")]

use std::time::Duration;

use oxvif::{
    analytics::MetadataDocument,
    metamorph::{
        ComparisonOptions, ComparisonOutcome, ComparisonProjection, ComparisonReason, CorpusLimits,
        EventWindow, EventWindowError, FixtureCorpus, FixtureStore, compare_corpora,
        compare_event_windows,
    },
    soap::SoapEnvelope,
    transport::Transport,
};

const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";
const ANALYTICS: &str = "http://www.onvif.org/ver20/analytics/wsdl";
const SEARCH: &str = "http://www.onvif.org/ver10/search/wsdl";
const PTZ: &str = "http://www.onvif.org/ver20/ptz/wsdl";
const T: &str = "http://www.onvif.org/ver10/schema";
const MOCK: &str = "http://mock";

fn xml(ns: &str, tag: &str, fields: &str) -> String {
    SoapEnvelope::new(format!("<x:{tag} xmlns:x=\"{ns}\">{fields}</x:{tag}>")).build()
}

fn record(store: &mut FixtureStore, ns: &str, op: &str, request: &str, reply: &str) {
    store.record(
        &format!("{ns}/{op}"),
        &xml(ns, op, request),
        &xml(ns, &format!("{op}Response"), reply),
    );
}

fn corpus(store: &FixtureStore) -> FixtureCorpus {
    let dir = tempfile::tempdir().unwrap();
    store.save(dir.path()).unwrap();
    FixtureCorpus::load(dir.path(), CorpusLimits::default()).unwrap()
}

#[tokio::test]
async fn selected_projections_use_full_actions_and_omit_private_wire_values() {
    let mut left = FixtureStore::new("left");
    let mut right = FixtureStore::new("right");
    for (store, quota, stamp, uri) in [
        (
            &mut left,
            4,
            "2026-01-01T00:00:00Z",
            "rtsp://u:p@left.test/live?secret=query-1",
        ),
        (
            &mut right,
            8,
            "2026-01-02T00:00:00Z",
            "rtsp://u:p@right.test/other?secret=query-2",
        ),
    ] {
        record(
            store,
            MEDIA2,
            "GetStreamUri",
            "<x:Protocol>RTSP</x:Protocol><x:ProfileToken>p</x:ProfileToken>",
            &format!("<x:Uri>{uri}</x:Uri>"),
        );
        record(
            store,
            MEDIA2,
            "GetOSDOptions",
            "<x:ConfigurationToken>cfg</x:ConfigurationToken>",
            &format!(
                "<x:OSDOptions><tt:MaximumNumberOfOSDs Total=\"{quota}\" PlainText=\"1\"/><tt:Type>Text</tt:Type><tt:PositionOption>UpperLeft</tt:PositionOption></x:OSDOptions>"
            ),
        );
        record(
            store,
            MEDIA2,
            "GetMaskOptions",
            "<x:ConfigurationToken>cfg</x:ConfigurationToken>",
            "<x:Options RectangleOnly=\"1\"><x:MaxMasks>4</x:MaxMasks><x:MaxPoints>4</x:MaxPoints><x:Types>Color</x:Types><x:Color><tt:ColorList X=\"1\" Y=\"0\" Z=\"0\"/></x:Color></x:Options>",
        );
        record(
            store,
            MEDIA2,
            "GetAudioOutputConfigurationOptions",
            "<x:ConfigurationToken>cfg</x:ConfigurationToken>",
            "<x:Options><tt:OutputTokensAvailable>out</tt:OutputTokensAvailable><tt:OutputLevelRange><tt:Min>-10</tt:Min><tt:Max>10</tt:Max></tt:OutputLevelRange></x:Options>",
        );
        record(
            store,
            ANALYTICS,
            "GetServiceCapabilities",
            "",
            "<x:Capabilities RuleSupport=\"true\" AnalyticsModuleSupport=\"false\"/>",
        );
        record(
            store,
            SEARCH,
            "GetRecordingSummary",
            "",
            &format!(
                "<x:Summary><tt:DataFrom>{stamp}</tt:DataFrom><tt:DataUntil>{stamp}</tt:DataUntil><tt:NumberRecordings>1</tt:NumberRecordings></x:Summary>"
            ),
        );
        record(
            store,
            PTZ,
            "GetConfigurationOptions",
            "<x:ConfigurationToken>cfg</x:ConfigurationToken>",
            "<x:PTZConfigurationOptions><tt:Spaces><tt:AbsoluteZoomPositionSpace><tt:URI>urn:zoom</tt:URI></tt:AbsoluteZoomPositionSpace></tt:Spaces></x:PTZConfigurationOptions>",
        );
        record(
            store,
            SEARCH,
            "FindRecordings",
            "<x:Scope/><x:KeepAliveTime>PT60S</x:KeepAliveTime>",
            "<x:SearchToken>old</x:SearchToken>",
        );
    }
    let left = corpus(&left);
    let right = corpus(&right);
    let report = compare_corpora(&left, &right, &ComparisonOptions::default())
        .await
        .unwrap();
    assert_eq!(report.rows.len(), 8);
    for projection in [
        ComparisonProjection::StreamUriScheme,
        ComparisonProjection::MaskOptions,
        ComparisonProjection::AudioOutputOptions,
        ComparisonProjection::AnalyticsSupport,
        ComparisonProjection::RecordingSummary,
        ComparisonProjection::PtzSpaces,
    ] {
        let row = report
            .rows
            .iter()
            .find(|row| row.projection == Some(projection))
            .unwrap();
        assert_eq!(row.outcome, ComparisonOutcome::Equal, "{row:?}");
    }
    let changed = report
        .rows
        .iter()
        .find(|row| row.projection == Some(ComparisonProjection::OsdOptions))
        .unwrap();
    assert_eq!(changed.outcome, ComparisonOutcome::Different);
    assert_eq!(changed.changed_fields, ["/max_total"]);
    assert!(
        changed
            .unexamined_left
            .iter()
            .any(|field| field.ends_with("PlainText"))
    );
    let uri = report
        .rows
        .iter()
        .find(|row| row.projection == Some(ComparisonProjection::StreamUriScheme))
        .unwrap();
    assert!(
        uri.unexamined_left
            .iter()
            .any(|field| field.ends_with("Uri"))
    );
    let stateful = report
        .rows
        .iter()
        .find(|row| row.action.ends_with("FindRecordings"))
        .unwrap();
    assert_eq!(
        (stateful.outcome, stateful.reason),
        (
            ComparisonOutcome::Unsupported,
            ComparisonReason::StatefulWorkflow
        )
    );
    let serialized = serde_json::to_string(&report).unwrap();
    for secret in ["left.test", "right.test", "query-1", "query-2"] {
        assert!(!serialized.contains(secret));
    }
}

fn notifications(prefix: &str, ns: &str, times: &[&str]) -> MetadataDocument {
    let values = times.iter().map(|time| {
        format!("<n:NotificationMessage><n:Topic xmlns:{prefix}=\"{ns}\" Dialect=\"http://docs.oasis-open.org/wsn/t-1/TopicExpression/Concrete\">{prefix}:RuleEngine/Motion</n:Topic><n:Message><tt:Message UtcTime=\"{time}\" PropertyOperation=\"Changed\"/></n:Message></n:NotificationMessage>")
    }).collect::<String>();
    MetadataDocument::parse(&format!("<tt:MetadataStream xmlns:tt=\"{T}\" xmlns:n=\"http://docs.oasis-open.org/wsn/b-2\"><tt:Event>{values}</tt:Event></tt:MetadataStream>")).unwrap()
}

#[test]
fn event_windows_keep_order_and_compare_relative_times_and_expanded_topics() {
    let left = notifications(
        "a",
        "urn:same",
        &["2026-01-01T00:00:01Z", "2026-01-01T00:00:02Z"],
    );
    let right = notifications(
        "b",
        "urn:same",
        &["2026-01-01T10:00:01.050+01:00", "2026-01-01T10:00:03+01:00"],
    );
    let left_start = "2026-01-01T00:00:00Z".parse().unwrap();
    let left_end = "2026-01-01T00:01:00Z".parse().unwrap();
    let right_start = "2026-01-01T09:00:00Z".parse().unwrap();
    let right_end = "2026-01-01T09:01:00Z".parse().unwrap();
    let comparison = compare_event_windows(
        EventWindow {
            start: &left_start,
            end: &left_end,
            notifications: &left.notifications,
        },
        EventWindow {
            start: &right_start,
            end: &right_end,
            notifications: &right.notifications,
        },
        Duration::from_millis(100),
    )
    .unwrap();
    assert_eq!(comparison.differences.len(), 1);
    assert_eq!(comparison.differences[0].index, 1);
    assert!(comparison.differences[0].timing);
    assert!(!comparison.differences[0].identity);
    let other = notifications(
        "a",
        "urn:different",
        &["2026-01-01T00:00:02Z", "2026-01-01T00:00:01Z"],
    );
    let result = compare_event_windows(
        EventWindow {
            start: &left_start,
            end: &left_end,
            notifications: &left.notifications,
        },
        EventWindow {
            start: &left_start,
            end: &left_end,
            notifications: &other.notifications,
        },
        Duration::ZERO,
    );
    assert_eq!(result, Err(EventWindowError::InvalidTime(1)));
}

#[tokio::test]
async fn mask_replay_retires_only_after_a_committed_change() {
    let mut store = FixtureStore::new("mask evidence");
    let scope = "<x:ConfigurationToken>VSC_1</x:ConfigurationToken>";
    record(
        &mut store,
        MEDIA2,
        "GetMasks",
        scope,
        "<x:Masks token=\"old-mask\"><x:ConfigurationToken>VSC_1</x:ConfigurationToken><x:Polygon><tt:Point x=\"0\" y=\"0\"/><tt:Point x=\"1\" y=\"0\"/><tt:Point x=\"0\" y=\"1\"/></x:Polygon><x:Type>Blurred</x:Type><x:Enabled>true</x:Enabled></x:Masks>",
    );
    let replay = oxvif::metamorph::MetamorphTransport::new(store);
    let read_action = format!("{MEDIA2}/GetMasks");
    let read = || replay.soap_post(MOCK, &read_action, xml(MEDIA2, "GetMasks", scope));
    assert!(read().await.unwrap().contains("old-mask"));
    let create = format!("{MEDIA2}/CreateMask");
    let request = "<x:Mask><x:ConfigurationToken>VSC_1</x:ConfigurationToken><x:Polygon><tt:Point x=\"0\" y=\"0\"/><tt:Point x=\"1\" y=\"0\"/><tt:Point x=\"0\" y=\"1\"/></x:Polygon><x:Type>Blurred</x:Type><x:Enabled>true</x:Enabled></x:Mask>";
    let invalid = request.replace("Blurred", "Unmodeled");
    assert!(
        replay
            .soap_post(MOCK, &create, xml(MEDIA2, "CreateMask", &invalid))
            .await
            .unwrap()
            .contains("Fault")
    );
    assert!(read().await.unwrap().contains("old-mask"));
    assert!(
        replay
            .soap_post(MOCK, &create, xml(MEDIA2, "CreateMask", request))
            .await
            .unwrap()
            .contains("MASK_1")
    );
    let current = read().await.unwrap();
    assert!(current.contains("MASK_1"));
    assert!(!current.contains("old-mask"));
}
