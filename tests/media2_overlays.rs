#![cfg(feature = "mock")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use oxvif::{
    OnvifClient,
    mock::{MockState, MockTransport},
    overlays::{MaskKind, MaskPoint, MaskSettings, OverlayWriteError},
    transport::{Transport, TransportError},
    types::{OsdConfiguration, OsdPosition, OsdTextString},
};

const MEDIA: &str = "http://mock/media2";
const NS: &str = "http://www.onvif.org/ver20/media/wsdl";

struct Capture {
    mock: MockTransport,
    lost_creates: AtomicUsize,
    requests: AtomicUsize,
}

#[async_trait::async_trait]
impl Transport for Capture {
    async fn soap_post(
        &self,
        url: &str,
        action: &str,
        body: String,
    ) -> Result<String, TransportError> {
        let ordinal = self.requests.fetch_add(1, Ordering::SeqCst);
        let operation = action
            .strip_prefix(NS)
            .and_then(|value| value.strip_prefix('/'));
        if let Some(operation) = operation {
            assert_eq!(url, MEDIA);
            let mut reader = quick_xml::NsReader::from_str(&body);
            loop {
                let (binding, event) = reader.read_resolved_event().unwrap();
                match event {
                    quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag)
                        if tag.local_name().as_ref() == operation =>
                    {
                        assert!(
                            matches!(binding, quick_xml::name::ResolveResult::Bound(ns) if ns.as_ref() == NS)
                        );
                        break;
                    }
                    quick_xml::events::Event::Eof => panic!("missing operation {operation}"),
                    _ => {}
                }
            }
            if let Some(directory) = std::env::var_os("OXVIF_OVERLAY_WIRE") {
                std::fs::write(
                    std::path::Path::new(&directory).join(format!("{ordinal:03}-{operation}.xml")),
                    &body,
                )
                .unwrap();
            }
        }
        let result = self.mock.soap_post(url, action, body).await;
        if operation == Some("CreateMask")
            && self
                .lost_creates
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                    value.checked_sub(1)
                })
                .is_ok()
        {
            return Err(TransportError::Deadline);
        }
        result
    }
}

fn osd(config: &str) -> OsdConfiguration {
    OsdConfiguration {
        token: String::new(),
        video_source_config_token: config.into(),
        type_: "Text".into(),
        position: OsdPosition {
            type_: "UpperLeft".into(),
            ..Default::default()
        },
        text_string: Some(OsdTextString {
            type_: "Plain".into(),
            plain_text: Some(" A & B < C ".into()),
            ..Default::default()
        }),
        image_path: None,
    }
}

fn mask() -> MaskSettings {
    MaskSettings {
        polygon: vec![
            MaskPoint { x: -0.5, y: -0.5 },
            MaskPoint { x: 0.5, y: -0.5 },
            MaskPoint { x: 0.0, y: 0.5 },
        ],
        kind: MaskKind::Blurred,
        color: None,
        enabled: true,
    }
}

#[tokio::test]
async fn media2_overlays_are_scoped_and_reconcile_lost_replies() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let counter = hooks.clone();
    let mut mock = MockState::new();
    mock.set_on_change(Arc::new(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    let capture = Arc::new(Capture {
        mock: MockTransport::with_state(mock),
        lost_creates: AtomicUsize::new(0),
        requests: AtomicUsize::new(0),
    });
    let client = OnvifClient::new("http://mock/device").with_transport(capture.clone());
    let first = client.media2_overlays(MEDIA, "VSC_1").unwrap();
    let second = client.media2_overlays(MEDIA, "VSC_2").unwrap();
    let capabilities = second.capabilities().await.unwrap();
    assert_eq!(
        (capabilities.osd, capabilities.mask),
        (Some(true), Some(true))
    );
    let osd_limit = second.osd_options().await.unwrap().max_per_text_type["Plain"];
    assert!(osd_limit > 0);
    let first_osds = first.osds().await.unwrap();
    let created = second.create_osd(&osd("VSC_2")).await.unwrap();
    let mut observed = created.observed.unwrap();
    assert_eq!(
        observed.text_string.as_ref().unwrap().plain_text,
        osd("VSC_2").text_string.unwrap().plain_text
    );
    assert!(first.osd(&created.token).await.is_err());
    assert_eq!(first.osds().await.unwrap(), first_osds);
    let shared = client
        .get_osd("http://mock/media", &created.token)
        .await
        .unwrap();
    assert_eq!(shared.token, created.token);
    observed.position.type_ = "LowerLeft".into();
    assert_eq!(
        second.set_osd(&observed).await.unwrap().observed.unwrap(),
        observed
    );
    assert_eq!(hooks.load(Ordering::SeqCst), 2);
    let mut unrelated = observed.clone();
    unrelated.video_source_config_token = "VSC_1".into();
    assert!(matches!(
        second.set_osd(&unrelated).await,
        Err(OverlayWriteError::Preflight(_))
    ));
    assert_eq!(hooks.load(Ordering::SeqCst), 2);
    assert!(
        second
            .delete_osd(&created.token)
            .await
            .unwrap()
            .observed
            .unwrap()
    );
    assert_eq!(hooks.load(Ordering::SeqCst), 3);

    let mut bow_tie = mask();
    bow_tie.polygon = vec![
        MaskPoint { x: -1.0, y: -1.0 },
        MaskPoint { x: 1.0, y: 1.0 },
        MaskPoint { x: -1.0, y: 1.0 },
        MaskPoint { x: 1.0, y: -1.0 },
    ];
    assert!(matches!(
        first.create_mask(&bow_tie).await,
        Err(OverlayWriteError::Preflight(_))
    ));
    let limit = first.mask_options().await.unwrap().max_masks;
    let mut records = Vec::new();
    for _ in 0..limit {
        let written = first.create_mask(&mask()).await.unwrap();
        records.push(written.observed.unwrap());
    }
    assert!(matches!(
        first.create_mask(&mask()).await,
        Err(OverlayWriteError::Preflight(_))
    ));
    assert_eq!(first.masks().await.unwrap(), records);
    assert!(second.masks().await.unwrap().is_empty());
    assert!(second.mask(&records[0].token).await.is_err());
    let mut updated = mask();
    updated.kind = MaskKind::Pixelated;
    updated.enabled = false;
    assert_eq!(
        first
            .set_mask(&records[0].token, &updated)
            .await
            .unwrap()
            .observed
            .unwrap()
            .settings,
        updated
    );
    assert_eq!(first.mask(&records[1].token).await.unwrap(), records[1]);
    assert!(
        first
            .delete_mask(&records[0].token)
            .await
            .unwrap()
            .observed
            .unwrap()
    );

    capture.lost_creates.store(1, Ordering::SeqCst);
    assert!(matches!(
        second.create_mask(&mask()).await,
        Err(OverlayWriteError::OutcomeUnknown(_))
    ));
    assert_eq!(second.masks().await.unwrap().len(), 1);
    assert_eq!(hooks.load(Ordering::SeqCst), 3);
    let independent =
        OnvifClient::new("http://mock").with_transport(Arc::new(MockTransport::new()));
    assert!(
        independent
            .media2_overlays(MEDIA, "VSC_1")
            .unwrap()
            .masks()
            .await
            .unwrap()
            .is_empty()
    );
}
