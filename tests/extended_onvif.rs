use std::{
    collections::VecDeque,
    future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use oxvif::{
    OnvifClient,
    analytics::{
        AnalyticsKind, AnalyticsParameter, AnalyticsParameterDescription, MetadataDocument,
    },
    audio_configuration::{AudioConfigurationOptions, AudioFamily, AudioSettings},
    configuration_write::ConfigurationWriteError,
    events::EventItem,
    media_workflow::MediaService,
    recording::{
        EventSearchRequest, EventTopic, ExportPollError, ExportPollOptions, ExportRequest,
        RecordingScope, RecordingSearchCleanup, RecordingSearchOptions, RecordingSearchStopReason,
        RecordingTimestamp,
    },
    transport::{Transport, TransportError},
};

const DEVICE: &str = "http://camera.test/device";
const SERVICE: &str = "http://camera.test/service";
const MEDIA1: &str = "http://www.onvif.org/ver10/media/wsdl";
const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";
const SEARCH: &str = "http://www.onvif.org/ver10/search/wsdl";
const RECORDING: &str = "http://www.onvif.org/ver10/recording/wsdl";
const ANALYTICS: &str = "http://www.onvif.org/ver20/analytics/wsdl";
const T: &str = "http://www.onvif.org/ver10/schema";
const DATE: &str = "2026-11-11T08:00:00+08:00";
const END: &str = "2026-11-11T00:01:00Z";

struct Exchange {
    namespace: &'static str,
    name: &'static str,
    required: Vec<&'static str>,
    absent: Vec<&'static str>,
    reply: String,
}

fn step(
    namespace: &'static str,
    name: &'static str,
    required: &[&'static str],
    fields: &str,
) -> Exchange {
    Exchange {
        namespace,
        name,
        required: required.into(),
        absent: Vec::new(),
        reply: format!("<v:{name}Response>{fields}</v:{name}Response>"),
    }
}

fn fault(mut exchange: Exchange) -> Exchange {
    exchange.reply = "<s:Fault><s:Code><s:Value>s:Sender</s:Value><s:Subcode><s:Value>e:ActionNotSupported</s:Value></s:Subcode></s:Code><s:Reason><s:Text xml:lang=\"en\">refused</s:Text></s:Reason></s:Fault>".into();
    exchange
}

#[derive(Default)]
struct Script(Mutex<VecDeque<Exchange>>);

impl Script {
    fn client(steps: Vec<Exchange>) -> (Arc<Self>, OnvifClient) {
        let script = Arc::new(Self(Mutex::new(steps.into())));
        let client = OnvifClient::new(DEVICE).with_transport(script.clone());
        (script, client)
    }

    fn done(&self) {
        assert!(self.0.lock().unwrap().is_empty());
    }
}

#[async_trait::async_trait]
impl Transport for Script {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let exchange = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected exchange {action}"));
        assert_eq!(url, SERVICE);
        assert_eq!(action, format!("{}/{}", exchange.namespace, exchange.name));
        let mut reader = quick_xml::NsReader::from_str(&body);
        loop {
            let (binding, event) = reader.read_resolved_event().unwrap();
            match event {
                quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag)
                    if tag.local_name().as_ref() == exchange.name =>
                {
                    assert!(
                        matches!(binding, quick_xml::name::ResolveResult::Bound(ns) if ns.as_ref() == exchange.namespace)
                    );
                    break;
                }
                quick_xml::events::Event::Eof => panic!("operation not found"),
                _ => {}
            }
        }
        for field in exchange.required {
            assert!(body.contains(field), "missing {field} in {body}");
        }
        for field in exchange.absent {
            assert!(!body.contains(field), "unexpected {field} in {body}");
        }
        if let Some(directory) = std::env::var_os("OXVIF_EXTENDED_WIRE") {
            static ORDINAL: AtomicUsize = AtomicUsize::new(0);
            let ordinal = ORDINAL.fetch_add(1, Ordering::Relaxed);
            std::fs::write(
                std::path::Path::new(&directory)
                    .join(format!("{ordinal:03}-{}.xml", exchange.name)),
                body,
            )
            .unwrap();
        }
        Ok(format!(
            "<s:Envelope xmlns:s=\"http://www.w3.org/2003/05/soap-envelope\" xmlns:v=\"{}\" xmlns:t=\"{T}\" xmlns:e=\"http://www.onvif.org/ver10/error\" xmlns:n=\"http://docs.oasis-open.org/wsn/b-2\"><s:Body>{}</s:Body></s:Envelope>",
            exchange.namespace, exchange.reply
        ))
    }
}

#[tokio::test]
async fn audio_bindings_keep_configuration_profile_and_port_identities() {
    const OUTPUT: &str = "<t:OutputToken>speaker-7</t:OutputToken><t:SendPrimacy>urn:camera:server</t:SendPrimacy><t:OutputLevel>-12</t:OutputLevel>";
    const OUTPUT_OPTIONS: &str = "<v:Options><t:OutputTokensAvailable>speaker-7</t:OutputTokensAvailable><t:SendPrimacyOptions>urn:camera:server</t:SendPrimacyOptions><t:OutputLevelRange><t:Min>-20</t:Min><t:Max>10</t:Max></t:OutputLevelRange></v:Options>";
    for (binding, ns, get, item, compatible) in [
        (
            MediaService::Media1,
            MEDIA1,
            "GetAudioOutputConfiguration",
            "Configuration",
            "GetCompatibleAudioOutputConfigurations",
        ),
        (
            MediaService::Media2,
            MEDIA2,
            "GetAudioOutputConfigurations",
            "Configurations",
            "GetAudioOutputConfigurations",
        ),
    ] {
        let configuration = |tag: &str, name: &str| {
            format!(
                "<v:{tag} token=\"out-cfg\"><t:Name>{name}</t:Name><t:UseCount>2</t:UseCount>{OUTPUT}</v:{tag}>"
            )
        };
        let get_fields = ["<ConfigurationToken>out-cfg</ConfigurationToken>"];
        let option_fields =
            ["<ConfigurationToken>out-cfg</ConfigurationToken><ProfileToken>p-1</ProfileToken>"];
        let mut set = step(
            ns,
            "SetAudioOutputConfiguration",
            &[
                "<Configuration token=\"out-cfg\">",
                "<tt:UseCount>2</tt:UseCount>",
                "<tt:OutputToken>speaker-7</tt:OutputToken>",
                "<tt:SendPrimacy>urn:camera:server</tt:SendPrimacy><tt:OutputLevel>-12</tt:OutputLevel>",
            ],
            "",
        );
        if binding == MediaService::Media1 {
            set.required
                .push("<ForcePersistence>true</ForcePersistence>");
        } else {
            set.absent.push("ForcePersistence");
        }
        let (script, client) = Script::client(vec![
            step(
                ns,
                compatible,
                &["<ProfileToken>p-1</ProfileToken>"],
                &configuration("Configurations", "old"),
            ),
            step(ns, get, &get_fields, &configuration(item, "old")),
            step(
                ns,
                "GetAudioOutputConfigurationOptions",
                &option_fields,
                OUTPUT_OPTIONS,
            ),
            step(ns, get, &get_fields, &configuration(item, "old")),
            step(
                ns,
                "GetAudioOutputConfigurationOptions",
                &option_fields,
                OUTPUT_OPTIONS,
            ),
            set,
            step(ns, get, &get_fields, &configuration(item, "new")),
        ]);
        let audio = client.audio_configurations(SERVICE, binding).unwrap();
        assert_eq!(
            audio.list(AudioFamily::Output, Some("p-1")).await.unwrap()[0].token,
            "out-cfg"
        );
        let mut settings = AudioSettings::Output {
            output_token: "speaker-7".into(),
            send_primacy: Some("urn:camera:server".into()),
            level: -30,
        };
        assert!(matches!(
            audio.set("out-cfg", "new", &settings, Some("p-1")).await,
            Err(ConfigurationWriteError::Preflight(_))
        ));
        if let AudioSettings::Output { level, .. } = &mut settings {
            *level = -12;
        }
        let result = audio
            .set("out-cfg", "new", &settings, Some("p-1"))
            .await
            .unwrap()
            .observed
            .unwrap();
        assert_eq!(result.name, "new");
        assert_eq!(result.settings, settings);
        script.done();
    }
}

#[tokio::test]
async fn source_and_decoder_are_independent_and_unknown_fields_block_writes() {
    let (script, client) = Script::client(vec![
        step(
            MEDIA2,
            "GetAudioSourceConfigurations",
            &["source-cfg"],
            "<v:Configurations token=\"source-cfg\"><t:Name>mic</t:Name><t:UseCount>1</t:UseCount><t:SourceToken>mic-2</t:SourceToken></v:Configurations>",
        ),
        step(
            MEDIA2,
            "GetAudioSourceConfigurationOptions",
            &["source-cfg"],
            "<v:Options><t:InputTokensAvailable>mic-3</t:InputTokensAvailable></v:Options>",
        ),
        step(
            MEDIA2,
            "SetAudioSourceConfiguration",
            &["<tt:SourceToken>mic-3</tt:SourceToken>"],
            "",
        ),
        step(
            MEDIA2,
            "GetAudioSourceConfigurations",
            &["source-cfg"],
            "<v:Configurations token=\"source-cfg\"><t:Name>microphone</t:Name><t:UseCount>1</t:UseCount><t:SourceToken>mic-3</t:SourceToken></v:Configurations>",
        ),
        step(
            MEDIA2,
            "GetAudioDecoderConfigurationOptions",
            &[],
            "<v:Options><t:Encoding>PCMU</t:Encoding><t:BitrateList><t:Items>64</t:Items></t:BitrateList><t:SampleRateList><t:Items>8</t:Items></t:SampleRateList></v:Options>",
        ),
        step(
            MEDIA2,
            "GetAudioDecoderConfigurations",
            &["decoder"],
            "<v:Configurations token=\"decoder\" xmlns:x=\"urn:vendor\" x:private=\"keep\"><t:Name>old</t:Name><t:UseCount>1</t:UseCount></v:Configurations>",
        ),
        step(
            MEDIA1,
            "GetAudioDecoderConfigurationOptions",
            &[],
            "<v:Options><t:G711DecOptions><t:Bitrate><t:Items>64</t:Items></t:Bitrate><t:SampleRateRange><t:Items>8</t:Items></t:SampleRateRange></t:G711DecOptions></v:Options>",
        ),
    ]);
    let audio = client
        .audio_configurations(SERVICE, MediaService::Media2)
        .unwrap();
    let source = AudioSettings::Source {
        source_token: "mic-3".into(),
    };
    assert_eq!(
        audio
            .set("source-cfg", "microphone", &source, None)
            .await
            .unwrap()
            .observed
            .unwrap()
            .settings,
        source
    );
    let AudioConfigurationOptions::Decoder(media2) = audio
        .options(AudioFamily::Decoder, None, None)
        .await
        .unwrap()
    else {
        panic!("decoder options")
    };
    assert_eq!(media2[0].encoding, "PCMU");
    assert!(matches!(
        audio
            .set("decoder", "replacement", &AudioSettings::Decoder, None)
            .await,
        Err(ConfigurationWriteError::Preflight(_))
    ));
    let media1 = client
        .audio_configurations(SERVICE, MediaService::Media1)
        .unwrap();
    let AudioConfigurationOptions::Decoder(media1) = media1
        .options(AudioFamily::Decoder, None, None)
        .await
        .unwrap()
    else {
        panic!("decoder options")
    };
    assert_eq!(
        (
            media1[0].encoding.as_str(),
            media1[0].sample_rates.as_slice()
        ),
        ("G711", &[8][..])
    );
    script.done();
}

#[tokio::test]
async fn analytics_preserves_qnames_descriptors_and_vendor_items() {
    let (script, client) = Script::client(vec![
        step(
            ANALYTICS,
            "GetServiceCapabilities",
            &[],
            "<v:Capabilities RuleSupport=\"1\" AnalyticsModuleSupport=\"true\" SupportedMetadata=\"false\"/>",
        ),
        step(
            ANALYTICS,
            "GetRules",
            &["<ConfigurationToken>cfg</ConfigurationToken>"],
            "<v:Rule xmlns:x=\"urn:one\" Name=\"a\" Type=\"x:Motion\"><t:Parameters><t:SimpleItem Name=\"first\" Value=\"7\"/><t:ElementItem Name=\"second\"><x:Value/></t:ElementItem><t:Extension><x:Extra/></t:Extension></t:Parameters></v:Rule><v:Rule xmlns:x=\"urn:two\" Name=\"b\" Type=\"x:Motion\"><t:Parameters/></v:Rule>",
        ),
        step(
            ANALYTICS,
            "GetSupportedRules",
            &["cfg"],
            "<v:SupportedRules Limit=\"4\"><t:RuleContentSchemaLocation>https://invalid.example/vendor.xsd</t:RuleContentSchemaLocation><t:RuleDescription xmlns:x=\"urn:two\" xmlns:schema=\"http://www.w3.org/2001/XMLSchema\" Name=\"x:Motion\"><t:Parameters><t:SimpleItemDescription Name=\"Width\" Type=\"schema:int\"/><t:ElementItemDescription Name=\"Shape\" Type=\"x:Polygon\"/></t:Parameters></t:RuleDescription></v:SupportedRules>",
        ),
        step(
            ANALYTICS,
            "GetAnalyticsModules",
            &["cfg"],
            "<v:AnalyticsModule Name=\"invalid\" Type=\"missing:Shape\"><t:Parameters/></v:AnalyticsModule>",
        ),
        fault(step(
            ANALYTICS,
            "GetSupportedAnalyticsModules",
            &["cfg"],
            "",
        )),
    ]);
    let service = client.analytics(SERVICE).unwrap();
    assert_eq!(service.capabilities().await.unwrap().rules, Some(true));
    let rules = service
        .configurations(AnalyticsKind::Rule, "cfg")
        .await
        .unwrap();
    assert_ne!(rules[0].kind, rules[1].kind);
    assert!(matches!(
        rules[0].parameters[0],
        AnalyticsParameter::Value(EventItem::Simple { .. })
    ));
    assert!(matches!(
        rules[0].parameters[1],
        AnalyticsParameter::Value(EventItem::Element(_))
    ));
    assert!(matches!(
        rules[0].parameters[2],
        AnalyticsParameter::Extension(_)
    ));
    let catalogue = service.supported(AnalyticsKind::Rule, "cfg").await.unwrap();
    assert_eq!(catalogue.limit, Some(4));
    assert!(matches!(&catalogue.descriptions[0].parameters[1],
        AnalyticsParameterDescription::Item { value_type, element: true, .. } if value_type.namespace == "urn:two"));
    assert!(
        service
            .configurations(AnalyticsKind::Module, "cfg")
            .await
            .is_err()
    );
    assert!(
        matches!(service.supported(AnalyticsKind::Module, "cfg").await, Err(oxvif::OnvifError::Soap(error)) if error.action_not_supported())
    );
    script.done();
}

const NOTIFICATION: &str = "<n:Topic xmlns:topic=\"urn:events\">topic:RuleEngine/Motion</n:Topic><n:Message><t:Message UtcTime=\"2026-11-11T00:00:00Z\" PropertyOperation=\"Changed\"><t:Source><t:SimpleItem Name=\"Source\" Value=\"VSC_1\"/></t:Source><t:Data><t:SimpleItem Name=\"State\" Value=\"true\"/></t:Data></t:Message></n:Message>";

#[test]
fn metadata_stream_keeps_frames_and_event_order() {
    let source = format!(
        "<t:MetadataStream xmlns:t=\"{T}\" xmlns:n=\"http://docs.oasis-open.org/wsn/b-2\" xmlns:x=\"urn:vendor\"><t:VideoAnalytics><t:Frame UtcTime=\"2026-11-11T00:00:00Z\"/></t:VideoAnalytics><t:Event><n:NotificationMessage>{NOTIFICATION}</n:NotificationMessage></t:Event><t:Extension><x:Vendor/></t:Extension></t:MetadataStream>"
    );
    let report = MetadataDocument::parse(&source).unwrap();
    assert_eq!(report.xml.content().len(), 3);
    assert_eq!(report.notifications.len(), 1);
    assert!(MetadataDocument::parse(&source.replace(T, "urn:wrong")).is_err());
    if let Some(directory) = std::env::var_os("OXVIF_EXTENDED_WIRE") {
        std::fs::write(
            std::path::Path::new(&directory).join("../metadata-instance.xml"),
            source,
        )
        .unwrap();
    }
}

fn query() -> EventSearchRequest {
    EventSearchRequest {
        start: END.parse().unwrap(),
        end: Some(DATE.parse().unwrap()),
        scope: RecordingScope {
            recordings: vec!["rec<&".into()],
            sources: Vec::new(),
        },
        topic: Some(EventTopic::new("urn:events", "RuleEngine/Motion").unwrap()),
        include_start_state: true,
    }
}

#[tokio::test(start_paused = true)]
async fn recording_timeline_and_event_owner_accept_offsets_and_end_once() {
    let event = format!(
        "<t:Result><t:RecordingToken>rec&lt;&amp;</t:RecordingToken><t:TrackToken>track-1</t:TrackToken><t:Time>{DATE}</t:Time><t:Event>{NOTIFICATION}</t:Event><t:StartStateEvent>false</t:StartStateEvent></t:Result>"
    );
    let (script, client) = Script::client(vec![
        step(
            SEARCH,
            "GetRecordingSummary",
            &[],
            &format!(
                "<v:Summary><t:DataFrom>{DATE}</t:DataFrom><t:DataUntil>{END}</t:DataUntil><t:NumberRecordings>1</t:NumberRecordings></v:Summary>"
            ),
        ),
        step(
            SEARCH,
            "GetRecordingInformation",
            &["<RecordingToken>rec&lt;&amp;</RecordingToken>"],
            "<v:RecordingInformation><t:RecordingToken>rec&lt;&amp;</t:RecordingToken><t:Source><t:Name>camera</t:Name></t:Source><t:Content>Video</t:Content><t:RecordingStatus>Recording</t:RecordingStatus></v:RecordingInformation>",
        ),
        step(
            SEARCH,
            "GetMediaAttributes",
            &["<RecordingTokens>rec&lt;&amp;</RecordingTokens>"],
            &format!(
                "<v:MediaAttributes><t:RecordingToken>rec&lt;&amp;</t:RecordingToken><t:From>{DATE}</t:From><t:Until>{END}</t:Until></v:MediaAttributes>"
            ),
        ),
        step(
            SEARCH,
            "FindEvents",
            &[
                "<StartPoint>2026-11-11T00:01:00Z</StartPoint><EndPoint>2026-11-11T08:00:00+08:00</EndPoint>",
                "<tt:IncludedRecordings>rec&lt;&amp;</tt:IncludedRecordings>",
                "topic:RuleEngine/Motion",
                "<IncludeStartState>true</IncludeStartState>",
            ],
            "<v:SearchToken>job-1</v:SearchToken>",
        ),
        step(
            SEARCH,
            "GetEventSearchResults",
            &["<SearchToken>job-1</SearchToken>"],
            &format!(
                "<v:ResultList><t:SearchState>Searching</t:SearchState>{event}</v:ResultList>"
            ),
        ),
        step(
            SEARCH,
            "GetEventSearchResults",
            &["<SearchToken>job-1</SearchToken>"],
            "<v:ResultList><t:SearchState>Completed</t:SearchState></v:ResultList>",
        ),
        step(
            SEARCH,
            "EndSearch",
            &["<SearchToken>job-1</SearchToken>"],
            &format!("<v:Endpoint>{END}</v:Endpoint>"),
        ),
    ]);
    let timeline = client.recording_timeline(SERVICE).unwrap();
    let summary = timeline.summary().await.unwrap();
    assert_eq!(summary.count, 1);
    assert_eq!(
        (summary.until.instant() - summary.from.instant()).whole_seconds(),
        60
    );
    assert!(
        "2026-02-29T08:00:00Z"
            .parse::<RecordingTimestamp>()
            .is_err()
    );
    assert!(
        "2026-11-11T08:00:00+14:01"
            .parse::<RecordingTimestamp>()
            .is_err()
    );
    assert_eq!(
        timeline
            .information("rec<&")
            .await
            .unwrap()
            .information
            .source_name,
        "camera"
    );
    assert_eq!(
        timeline
            .media_attributes(&["rec<&".into()], &summary.from)
            .await
            .unwrap()
            .len(),
        1
    );
    let mut owner = timeline
        .event_search(query(), RecordingSearchOptions::default())
        .unwrap();
    let report = owner.run().await.unwrap();
    assert_eq!(report.pages(), 2);
    assert_eq!(report.events()[0].track_token.as_deref(), Some("track-1"));
    assert_eq!(report.events()[0].notification.data.len(), 1);
    assert!(
        matches!(report.cleanup(), RecordingSearchCleanup::Released { endpoint: Some(endpoint) } if endpoint == END)
    );
    assert!(matches!(
        &*owner.cancel_and_close().await,
        RecordingSearchCleanup::Released { .. }
    ));
    script.done();
}

#[tokio::test]
async fn event_search_bound_and_cleanup_failure_are_independent() {
    let (script, client) = Script::client(vec![
        step(
            SEARCH,
            "FindEvents",
            &[],
            "<v:SearchToken>job</v:SearchToken>",
        ),
        step(
            SEARCH,
            "GetEventSearchResults",
            &[],
            "<v:ResultList><t:SearchState>Searching</t:SearchState></v:ResultList>",
        ),
        fault(step(SEARCH, "EndSearch", &["job"], "")),
    ]);
    let timeline = client.recording_timeline(SERVICE).unwrap();
    let mut owner = timeline
        .event_search(
            query(),
            RecordingSearchOptions::default().with_limits(1, 3, 16384),
        )
        .unwrap();
    let failure = owner.run().await.unwrap_err();
    assert_eq!(
        failure.report.stop_reason(),
        RecordingSearchStopReason::PageLimit
    );
    assert!(matches!(
        failure.report.cleanup(),
        RecordingSearchCleanup::Failed(_)
    ));
    script.done();
}

#[tokio::test(start_paused = true)]
async fn export_job_progress_stop_and_storage_scope_are_explicit() {
    let (script, client) = Script::client(vec![
        step(
            RECORDING,
            "ExportRecordedData",
            &[
                "<SearchScope><tt:IncludedRecordings>rec-1</tt:IncludedRecordings></SearchScope>",
                "<tt:StorageToken>store-1</tt:StorageToken><tt:RelativePath>footage/day</tt:RelativePath>",
            ],
            "<v:OperationToken>export-1</v:OperationToken><v:FileNames>evidence.mp4</v:FileNames>",
        ),
        step(
            RECORDING,
            "GetExportRecordedDataState",
            &["export-1"],
            "<v:Progress>0.5</v:Progress><v:FileProgressStatus><t:FileProgress><t:FileName>evidence.mp4</t:FileName><t:Progress>0.5</t:Progress></t:FileProgress></v:FileProgressStatus>",
        ),
        step(
            RECORDING,
            "GetExportRecordedDataState",
            &["export-1"],
            "<v:Progress>NaN</v:Progress><v:FileProgressStatus/>",
        ),
        step(
            RECORDING,
            "StopExportRecordedData",
            &["export-1"],
            "<v:Progress>0.5</v:Progress><v:FileProgressStatus/>",
        ),
        fault(step(RECORDING, "ExportRecordedData", &["footage/day"], "")),
    ]);
    let mut request = ExportRequest {
        start: Some(DATE.parse().unwrap()),
        end: Some(END.parse().unwrap()),
        scope: RecordingScope {
            recordings: vec!["rec-1".into()],
            sources: vec![],
        },
        file_format: "MP4".into(),
        storage_token: "store-1".into(),
        relative_directory: Some("../escape".into()),
    };
    let exports = client.recording_exports(SERVICE).unwrap();
    assert!(matches!(
        exports.start(&request).await,
        Err(ConfigurationWriteError::Preflight(_))
    ));
    request.relative_directory = Some("footage/day".into());
    let mut job = exports.start(&request).await.unwrap();
    assert_eq!(job.initial_file_names().unwrap(), ["evidence.mp4"]);
    let options = ExportPollOptions {
        max_polls: 1,
        interval: Duration::from_millis(100),
        ..Default::default()
    };
    assert!(matches!(
        job.wait_until(options, future::pending()).await,
        Err(ExportPollError::PollLimit)
    ));
    assert!(job.progress().await.is_err());
    assert!(
        matches!(job.wait_until(options, future::ready(())).await, Err(ExportPollError::Cancelled { stop: Ok(report) }) if report.progress == 0.5)
    );
    assert!(job.cancel().await.is_err());
    assert!(matches!(
        exports.start(&request).await,
        Err(ConfigurationWriteError::Refused(_))
    ));
    script.done();
}
