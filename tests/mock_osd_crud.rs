//! Project-authored OS1 behavior controls; external schemas validate wire shape.
#![cfg(feature = "mock")]

use oxvif::{
    OnvifClient, OsdColor, OsdConfiguration, OsdPosition, OsdTextString,
    mock::{MockState, MockTransport},
    soap::{find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const M: &str = "http://www.onvif.org/ver10/media/wsdl";
const T: &str = "http://www.onvif.org/ver10/schema";
fn envelope(op: &str, fields: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{M}' xmlns:tt='{T}'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
    )
}
async fn post(t: &dyn Transport, url: &str, op: &str, fields: &str) -> String {
    t.soap_post(url, &format!("{M}/{op}"), envelope(op, fields))
        .await
        .unwrap()
}
fn candidate(source: &str) -> OsdConfiguration {
    OsdConfiguration {
        token: String::new(),
        video_source_config_token: source.into(),
        type_: "Text".into(),
        position: OsdPosition {
            type_: "Custom".into(),
            x: Some(-0.25),
            y: Some(0.75),
        },
        text_string: Some(OsdTextString {
            type_: "Plain".into(),
            plain_text: Some("OS1 &amp; <北> \"'".into()),
            date_format: None,
            time_format: None,
            font_size: Some(23),
            font_color: Some(OsdColor {
                x: 10.0,
                y: 20.0,
                z: 30.0,
                colorspace: Some("urn:os1:color&space".into()),
                transparent: Some(71.0),
            }),
            background_color: None,
            is_persistent_text: None,
        }),
        image_path: None,
    }
}
fn payload(token: &str, source: &str) -> String {
    format!(
        "<m:OSD token='{token}'><tt:VideoSourceConfigurationToken>{source}</tt:VideoSourceConfigurationToken><tt:Type>Text</tt:Type><tt:Position><tt:Type>UpperRight</tt:Type></tt:Position><tt:TextString><tt:Type>Plain</tt:Type><tt:PlainText>OS1 raw</tt:PlainText></tt:TextString></m:OSD>"
    )
}
fn fault(xml: &str, code: &str, subs: &[&str], reason: &str) {
    let body = parse_soap_body(xml).unwrap();
    let f = body.child("Fault").unwrap_or_else(|| panic!("{xml}"));
    let mut c = f.child("Code").unwrap();
    assert_eq!(c.child("Value").unwrap().text(), code, "{xml}");
    for expected in subs {
        c = c.child("Subcode").unwrap();
        assert_eq!(c.child("Value").unwrap().text(), *expected, "{xml}");
    }
    assert!(c.child("Subcode").is_none(), "{xml}");
    assert_eq!(
        f.child("Reason").unwrap().child("Text").unwrap().text(),
        reason,
        "{xml}"
    );
}
async fn exercise(t: Arc<dyn Transport>, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let client = OnvifClient::new(url).with_transport(t.clone());
    let pristine = serde_json::to_value(&*state.read()).unwrap();
    let before = hooks.load(Ordering::SeqCst);
    let mut config = candidate("VSC_2");
    config.token = client.create_osd(url, &config).await.unwrap();
    assert_eq!(config.token, "OSD_2");
    assert_eq!(client.get_osd(url, &config.token).await.unwrap(), config);
    assert_eq!(
        client.get_osds(url, Some("VSC_2")).await.unwrap(),
        vec![config.clone()]
    );
    assert_eq!(client.get_osds(url, Some("VSC_1")).await.unwrap().len(), 1);
    assert_eq!(client.get_osds(url, None).await.unwrap().len(), 2);
    let options = client.get_osd_options(url, "VSC_2").await.unwrap();
    assert_eq!(options.max_osd, 8);
    assert_eq!(options.max_per_text_type["Plain"], 7);
    assert_eq!(options.font_size_range, Some((8, 72)));
    assert_eq!(hooks.load(Ordering::SeqCst), before + 1);
    config.position.x = Some(0.5);
    config.text_string.as_mut().unwrap().plain_text = Some("changed & <value>".into());
    client.set_osd(url, &config).await.unwrap();
    assert_eq!(client.get_osd(url, &config.token).await.unwrap(), config);
    assert_eq!(hooks.load(Ordering::SeqCst), before + 2);

    let good = payload("OSD_2", "VSC_2");
    let invalid = [
        (
            "SetOSD",
            payload("OSD_2", "VSC_1"),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock OSD setting: immutable source binding",
        ),
        (
            "SetOSD",
            payload("OSD_absent", "VSC_2"),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:NoConfig"],
            "OSD configuration not found: OSD_absent",
        ),
        (
            "CreateOSD",
            payload("", "absent"),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:NoConfig"],
            "OSD configuration not found: absent",
        ),
        (
            "SetOSD",
            good.replace("UpperRight", "MiddleUnknown"),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock OSD setting: Position/Type",
        ),
        (
            "SetOSD",
            good.replace(
                "<tt:PlainText>",
                "<tt:FontSize>bad</tt:FontSize><tt:PlainText>",
            ),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:ConfigModify"],
            "Invalid mock OSD setting: FontSize",
        ),
        (
            "SetOSD",
            good.replace(
                "<tt:TextString>",
                "<tt:TextString IsPersistentText='false'>",
            ),
            "s:Sender",
            vec!["mock:UnmodeledEffect"],
            "Unmodeled OSD setting",
        ),
        (
            "SetOSD",
            good.replace("<tt:PlainText>", "<tt:BackgroundColor/><tt:PlainText>"),
            "s:Sender",
            vec!["mock:UnmodeledEffect"],
            "Unmodeled OSD setting",
        ),
        (
            "SetOSD",
            good.replace("<tt:PlainText>", "<tt:Extension/><tt:PlainText>"),
            "s:Sender",
            vec!["mock:UnmodeledEffect"],
            "Unmodeled OSD setting",
        ),
        (
            "SetOSD",
            good.replace(
                "<tt:Type>Text</tt:Type>",
                "<tt:Type>Text</tt:Type><tt:Type>Image</tt:Type>",
            ),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            "SetOSD",
            good.replace(
                "<tt:VideoSourceConfigurationToken>VSC_2</tt:VideoSourceConfigurationToken>",
                "",
            ),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            "SetOSD",
            format!("{good}{good}"),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            "SetOSD",
            good.replace("<m:OSD ", "<foreign:OSD xmlns:foreign='urn:os1:foreign' ")
                .replace("</m:OSD>", "</foreign:OSD>"),
            "s:Sender",
            vec!["mock:UnmodeledEffect"],
            "Unmodeled OSD setting",
        ),
        (
            "DeleteOSD",
            "<m:OSDToken>absent</m:OSDToken>".into(),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:NoConfig"],
            "OSD configuration not found: absent",
        ),
        (
            "DeleteOSD",
            "<m:OSDToken>OSD_2</m:OSDToken><m:OSDToken>OSD_1</m:OSDToken>".into(),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            "DeleteOSD",
            "<m:OSDToken><m:nested>OSD_2</m:nested></m:OSDToken>".into(),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
        (
            "GetOSDs",
            "<m:ConfigurationToken>absent</m:ConfigurationToken>".into(),
            "s:Sender",
            vec!["ter:InvalidArgVal", "ter:NoConfig"],
            "OSD configuration not found: absent",
        ),
        (
            "GetOSDOptions",
            String::new(),
            "s:Sender",
            vec!["ter:InvalidArgs"],
            "Invalid Args",
        ),
    ];
    let preserved = serde_json::to_value(&*state.read()).unwrap();
    for (op, fields, code, subs, reason) in invalid {
        fault(
            &post(t.as_ref(), url, op, &fields).await,
            code,
            &subs,
            reason,
        );
        assert_eq!(
            serde_json::to_value(&*state.read()).unwrap(),
            preserved,
            "{op}"
        );
        assert_eq!(hooks.load(Ordering::SeqCst), before + 2, "{op}");
    }
    // Header candidates cannot replace the direct operation; alternate prefixes
    // and CDATA retain the one decoded literal string.
    let wire = envelope(
        "SetOSD",
        &good.replace("OS1 raw", "<![CDATA[OS1 &amp; <literal>]]>"),
    )
    .replace(
        "<s:Body>",
        &format!(
            "<s:Header><m:decoy>{}</m:decoy></s:Header><s:Body>",
            payload("OSD_1", "VSC_1")
        ),
    )
    .replace("m:", "p:")
    .replace("xmlns:m", "xmlns:p");
    let result = t
        .soap_post(url, &format!("{M}/SetOSD"), wire)
        .await
        .unwrap();
    find_response(&parse_soap_body(&result).unwrap(), "SetOSDResponse").unwrap();
    assert_eq!(
        client
            .get_osd(url, "OSD_2")
            .await
            .unwrap()
            .text_string
            .unwrap()
            .plain_text
            .as_deref(),
        Some("OS1 &amp; <literal>")
    );
    assert_eq!(
        serde_json::to_value(&state.read().osd.osds[0]).unwrap(),
        pristine["osd"]["osds"][0]
    );
    client.delete_osd(url, "OSD_2").await.unwrap();
    assert!(
        client
            .get_osds(url, Some("VSC_2"))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(hooks.load(Ordering::SeqCst), before + 4);
    let mut image = candidate("VSC_2");
    image.type_ = "Image".into();
    image.text_string = None;
    image.image_path = Some("https://example.invalid/os1?a=1&b=2".into());
    image.token = client.create_osd(url, &image).await.unwrap();
    assert_eq!(image.token, "OSD_3");
    assert_eq!(client.get_osd(url, &image.token).await.unwrap(), image);
    let other = MockTransport::new();
    assert_eq!(
        serde_json::to_value(&*other.device().read()).unwrap(),
        pristine
    );
}

#[tokio::test]
async fn osd_crud_in_process() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let mut state = MockState::new();
    state.set_on_change(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    }));
    let t = MockTransport::with_state(state);
    exercise(Arc::new(t.clone()), "http://mock", t.device(), &hooks).await;
}
#[cfg(feature = "mock-server")]
#[tokio::test]
async fn osd_crud_http() {
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = hooks.clone();
    let s = oxvif::mock::MockServer::builder()
        .on_change(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }))
        .start()
        .await
        .unwrap();
    exercise(
        Arc::new(oxvif::transport::HttpTransport::default()),
        s.device_url(),
        s.device(),
        &hooks,
    )
    .await;
}

async fn capacity(t: Arc<dyn Transport>, url: &str, state: &MockState) {
    let c = OnvifClient::new(url).with_transport(t.clone());
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..24 {
        let c = c.clone();
        let url = url.to_owned();
        tasks.spawn(async move { c.create_osd(&url, &candidate("VSC_2")).await });
    }
    let mut tokens = std::collections::BTreeSet::new();
    let mut rejected = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(token) => {
                assert!(tokens.insert(token));
            }
            Err(e) => {
                assert_eq!(
                    e.to_string(),
                    "SOAP fault [s:Receiver]: OSD source capacity exceeded"
                );
                rejected += 1;
            }
        }
    }
    assert_eq!(tokens.len(), 7);
    assert_eq!(rejected, 17);
    let mut image = candidate("VSC_2");
    image.type_ = "Image".into();
    image.text_string = None;
    image.image_path = Some("urn:os1:image".into());
    let token = c.create_osd(url, &image).await.unwrap();
    let before = serde_json::to_value(&*state.read()).unwrap();
    let raw = payload("", "VSC_2").replace("<tt:Type>Plain</tt:Type>", "<tt:Type>Date</tt:Type>");
    fault(
        &post(t.as_ref(), url, "CreateOSD", &raw).await,
        "s:Receiver",
        &["ter:Action", "ter:MaxOSDs"],
        "OSD source capacity exceeded",
    );
    assert_eq!(
        c.create_osd(url, &image).await.unwrap_err().to_string(),
        "SOAP fault [s:Receiver]: OSD source capacity exceeded"
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    image.token = token;
    c.set_osd(url, &image).await.unwrap(); // replacing self at full capacity succeeds
    let mut changed = candidate("VSC_2");
    changed.token = image.token;
    let err = c.set_osd(url, &changed).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "SOAP fault [s:Sender]: Invalid mock OSD setting: source capacity"
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    assert_eq!(
        c.create_osd(url, &candidate("VSC_1")).await.unwrap(),
        "OSD_10"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn osd_capacity_is_atomic_per_source_for_text_and_images() {
    let t = MockTransport::new();
    capacity(Arc::new(t.clone()), "http://mock", t.device()).await;
}
#[cfg(feature = "mock-server")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn osd_http_capacity_is_atomic_per_source_for_text_and_images() {
    let s = oxvif::mock::MockServer::builder().start().await.unwrap();
    capacity(
        Arc::new(oxvif::transport::HttpTransport::default()),
        s.device_url(),
        s.device(),
    )
    .await;
}

#[tokio::test]
async fn osd_imported_counter_collision_and_overflow_preserve_state() {
    let t = MockTransport::new();
    t.device().modify(|s| s.osd.next_token_id = 1);
    let c = OnvifClient::new("http://mock").with_transport(Arc::new(t.clone()));
    assert_eq!(
        c.create_osd("http://mock", &candidate("VSC_2"))
            .await
            .unwrap(),
        "OSD_2"
    );
    t.device().modify(|s| s.osd.next_token_id = u32::MAX);
    let before = serde_json::to_value(&*t.device().read()).unwrap();
    fault(
        &post(&t, "http://mock", "CreateOSD", &payload("", "VSC_2")).await,
        "s:Receiver",
        &["mock:RequestPolicy"],
        "Invalid mock OSD snapshot",
    );
    assert_eq!(serde_json::to_value(&*t.device().read()).unwrap(), before);
}

#[cfg(feature = "metamorph")]
fn recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("os1-841");
    for (op, fields) in [
        ("GetOSD", "<m:OSDToken>OSD_1</m:OSDToken>"),
        ("GetOSDs", ""),
        (
            "GetOSDOptions",
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
        ),
    ] {
        store.record(
            &format!("{M}/{op}"),
            &envelope(op, fields),
            "<recorded-os1-841/>",
        );
    }
    store
}
#[cfg(feature = "metamorph")]
async fn replay(t: &dyn Transport, url: &str, op: &str) {
    let reads = [
        ("GetOSD", "<m:OSDToken>OSD_1</m:OSDToken>"),
        ("GetOSDs", ""),
    ];
    let bad = if op == "DeleteOSD" {
        "<m:OSDToken>absent</m:OSDToken>".to_owned()
    } else {
        payload("absent", "absent")
    };
    fault(
        &post(t, url, op, &bad).await,
        "s:Sender",
        &["ter:InvalidArgVal", "ter:NoConfig"],
        "OSD configuration not found: absent",
    );
    for (read, fields) in reads {
        assert_eq!(post(t, url, read, fields).await, "<recorded-os1-841/>");
    }
    let good = if op == "DeleteOSD" {
        "<m:OSDToken>OSD_1</m:OSDToken>".to_owned()
    } else {
        payload("OSD_1", "VSC_1")
    };
    let result = post(t, url, op, &good).await;
    find_response(&parse_soap_body(&result).unwrap(), &format!("{op}Response")).unwrap();
    let list = post(t, url, "GetOSDs", "").await;
    let body = parse_soap_body(&list).unwrap();
    let rows = find_response(&body, "GetOSDsResponse").unwrap();
    let expected = match op {
        "CreateOSD" => 2,
        "SetOSD" => 1,
        _ => 0,
    };
    assert_eq!(rows.children_named("OSDs").count(), expected);
    if op != "DeleteOSD" {
        let raw = post(t, url, reads[0].0, reads[0].1).await;
        let b = parse_soap_body(&raw).unwrap();
        let o = find_response(&b, "GetOSDResponse")
            .unwrap()
            .child("OSD")
            .unwrap();
        assert_eq!(o.attr("token"), Some("OSD_1"));
        if op == "SetOSD" {
            assert_eq!(
                o.path(&["TextString", "PlainText"]).unwrap().text(),
                "OS1 raw"
            );
        }
    }
    assert_eq!(
        post(
            t,
            url,
            "GetOSDOptions",
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>"
        )
        .await,
        "<recorded-os1-841/>"
    );
}
#[cfg(feature = "metamorph")]
#[tokio::test]
async fn osd_replay_only_retires_reads_after_commit() {
    for op in ["CreateOSD", "SetOSD", "DeleteOSD"] {
        replay(
            &oxvif::metamorph::MetamorphTransport::new(recordings()),
            "http://mock",
            op,
        )
        .await;
    }
}
#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn osd_http_replay_only_retires_reads_after_commit() {
    for op in ["CreateOSD", "SetOSD", "DeleteOSD"] {
        let server = oxvif::mock::MockServer::builder()
            .replay(recordings())
            .start()
            .await
            .unwrap();
        replay(
            &oxvif::transport::HttpTransport::default(),
            server.device_url(),
            op,
        )
        .await;
    }
}
