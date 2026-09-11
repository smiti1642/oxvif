//! Source-authored controls for modeled source configuration, not schema fixtures.
#![cfg(feature = "mock")]

use oxvif::{
    mock::{MockState, MockTransport},
    soap::parse_soap_body,
    transport::Transport,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
const TT: &str = "http://www.onvif.org/ver10/schema";

fn envelope(ns: &str, op: &str, fields: &str) -> String {
    format!(
        "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{ns}' xmlns:tt='{TT}'><s:Body><m:{op}>{fields}</m:{op}></s:Body></s:Envelope>"
    )
}

async fn post(t: &dyn Transport, url: &str, ns: &str, op: &str, fields: &str) -> String {
    t.soap_post(url, &format!("{ns}/{op}"), envelope(ns, op, fields))
        .await
        .unwrap()
}

fn config(name: &str, width: &str, height: &str) -> String {
    format!(
        "<m:Configuration token='VSC_1'><tt:Name>{name}</tt:Name><tt:UseCount>79</tt:UseCount><tt:SourceToken>VS_1</tt:SourceToken><tt:Bounds x='0' y='0' width='{width}' height='{height}'/></m:Configuration>"
    )
}

async fn fields(t: &dyn Transport, url: &str, state: &MockState, hooks: &AtomicUsize) {
    let before = serde_json::to_value(&*state.read()).unwrap();
    let initial = hooks.load(Ordering::SeqCst);
    let xml = post(
        t,
        url,
        M2,
        "SetVideoSourceConfiguration",
        &config("must-not-commit-931", "640", "bad-931"),
    )
    .await;
    let body = parse_soap_body(&xml).unwrap();
    assert_eq!(body.children[0].local_name, "Fault", "{xml}");
    let fault = &body.children[0];
    assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), "s:Sender");
    assert_eq!(
        fault
            .path(&["Code", "Subcode", "Subcode", "Value"])
            .unwrap()
            .text(),
        "ter:ConfigModify"
    );
    assert_eq!(
        fault.path(&["Reason", "Text"]).unwrap().text(),
        "Invalid mock source setting: Bounds/@height"
    );
    assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
    for ns in [M1, M2] {
        let persistence = if ns == M1 {
            "<m:ForcePersistence>false</m:ForcePersistence>"
        } else {
            ""
        };
        let valid = config("candidate-933", "800", "480");
        for (bad, leaf, reason) in [
            (
                valid.replace("height='480'", "height='-1'"),
                "ter:ConfigModify",
                "Invalid mock source setting: Bounds/@height",
            ),
            (
                valid.replace("VS_1", "missing-source-933"),
                "ter:ConfigModify",
                "Invalid mock source setting: SourceToken",
            ),
            (
                valid.replace("VSC_1", "missing-config-933"),
                "ter:NoConfig",
                "Source configuration not found: missing-config-933",
            ),
            (
                valid.replace("x='0'", "x='7'"),
                "ter:ConfigModify",
                "Invalid mock source setting: Bounds/@x (only zero origin is modeled)",
            ),
            (
                valid.replace("<tt:UseCount>79", "<tt:UseCount>bad-933"),
                "ter:ConfigModify",
                "Invalid mock source setting: UseCount",
            ),
        ] {
            let xml = post(
                t,
                url,
                ns,
                "SetVideoSourceConfiguration",
                &format!("{bad}{persistence}"),
            )
            .await;
            assert_fault(&xml, "ter:InvalidArgVal", Some(leaf), reason);
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), initial);
        }
        for bad in [
            valid.replace("</tt:Name>", "</tt:Name><tt:Name>duplicate</tt:Name>"),
            valid.replace("candidate-933", "<tt:Name>nested</tt:Name>"),
            format!("{valid}{valid}"),
            valid.replace("<tt:SourceToken>VS_1</tt:SourceToken>", ""),
            valid.replace(
                "<tt:SourceToken>VS_1</tt:SourceToken>",
                "<tt:SourceToken>VS_1</tt:SourceToken><tt:SourceToken>VS_2</tt:SourceToken>",
            ),
        ] {
            let xml = post(
                t,
                url,
                ns,
                "SetVideoSourceConfiguration",
                &format!("{bad}{persistence}"),
            )
            .await;
            assert_fault(&xml, "ter:InvalidArgs", None, "Invalid Args");
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), initial);
        }
        for bad in [
            valid.replace("</m:Configuration>", "<tt:Extension><tt:Rotate><tt:Mode>ON</tt:Mode></tt:Rotate></tt:Extension></m:Configuration>"),
            valid.replace("</m:Configuration>", "<v:Name xmlns:v='urn:test:vendor'>decoy</v:Name></m:Configuration>"),
            valid.replace("token='VSC_1'", "token='VSC_1' custom='value'"),
        ] {
            let xml = post(t, url, ns, "SetVideoSourceConfiguration", &format!("{bad}{persistence}")).await;
            assert_fault(&xml, "mock:UnmodeledEffect", None, "Unmodeled source configuration setting");
            assert_eq!(serde_json::to_value(&*state.read()).unwrap(), before);
            assert_eq!(hooks.load(Ordering::SeqCst), initial);
        }
    }
    // A typed positive clamps to the newly selected sensor, not the old crop.
    // The other configuration and the caller-authored reference counts stay intact.
    let original = serde_json::to_value(&state.read().video_source_configs[1]).unwrap();
    for (index, ns) in [M1, M2].into_iter().enumerate() {
        let persistence = if ns == M1 {
            "<m:ForcePersistence>1</m:ForcePersistence>"
        } else {
            ""
        };
        let request = config(" renamed &amp; &lt;北&gt; &#13;&#10;&#9; ", "3000", "2000")
            .replace("VS_1", "VS_2")
            .replace("token='VSC_1'", "token='VSC_1' ViewMode='readonly-ignored'");
        let xml = post(
            t,
            url,
            ns,
            "SetVideoSourceConfiguration",
            &format!("{request}{persistence}"),
        )
        .await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0].local_name,
            "SetVideoSourceConfigurationResponse"
        );
        {
            let snapshot = state.read();
            let entry = &snapshot.video_source_configs[0];
            assert_eq!(
                (
                    &*entry.name,
                    &*entry.source_token,
                    entry.width,
                    entry.height,
                    entry.use_count
                ),
                (" renamed & <北> \r\n\t ", "VS_2", 1280, 720, 2)
            );
            assert_eq!(
                serde_json::to_value(&snapshot.video_source_configs[1]).unwrap(),
                original
            );
        }
        assert_eq!(hooks.load(Ordering::SeqCst), initial + index + 1);
        for service in [M1, M2] {
            let xml = post(t, url, service, "GetVideoSourceConfigurations", "").await;
            assert!(
                xml.contains("<tt:Name> renamed &amp; &lt;北&gt; &#13;&#10;&#9; </tt:Name>"),
                "{xml}"
            );
            let parsed = parse_soap_body(&xml).unwrap();
            let entry = parsed.children[0]
                .children_named("Configurations")
                .find(|c| c.attr("token") == Some("VSC_1"))
                .unwrap();
            assert_eq!(entry.child("Bounds").unwrap().attr("width"), Some("1280"));
        }
    }
}

fn assert_fault(xml: &str, first: &str, last: Option<&str>, reason: &str) {
    let parsed = parse_soap_body(xml).unwrap();
    assert_eq!(parsed.children[0].local_name, "Fault", "{xml}");
    let fault = &parsed.children[0];
    assert_eq!(fault.path(&["Code", "Value"]).unwrap().text(), "s:Sender");
    assert_eq!(
        fault.path(&["Code", "Subcode", "Value"]).unwrap().text(),
        first
    );
    assert_eq!(
        fault
            .path(&["Code", "Subcode", "Subcode", "Value"])
            .map(|v| v.text()),
        last
    );
    assert_eq!(fault.path(&["Reason", "Text"]).unwrap().text(), reason);
}

fn hooked_state() -> (MockState, Arc<AtomicUsize>) {
    let mut state = MockState::new();
    let count = Arc::new(AtomicUsize::new(0));
    let captured = count.clone();
    state.set_on_change(Arc::new(move |_| {
        captured.fetch_add(1, Ordering::SeqCst);
    }));
    (state, count)
}

#[tokio::test]
async fn source_fields_and_atomicity() {
    let (state, hooks) = hooked_state();
    let t = MockTransport::with_state(state);
    fields(&t, "http://mock", t.device(), &hooks).await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_source_fields_and_atomicity() {
    let (state, hooks) = hooked_state();
    let initial = state.read().clone();
    let server = oxvif::mock::MockServer::builder()
        .initial_state(initial)
        .on_change({
            let hooks = hooks.clone();
            Arc::new(move |_| {
                hooks.fetch_add(1, Ordering::SeqCst);
            })
        })
        .start()
        .await
        .unwrap();
    fields(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
        server.device(),
        &hooks,
    )
    .await;
}

async fn selectors(t: &dyn Transport, url: &str) {
    let xml = post(
        t,
        url,
        M2,
        "SetVideoSourceConfiguration",
        &config("crop-932", "640", "360"),
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoSourceConfigurationResponse"
    );
    for ns in [M1, M2] {
        let xml = post(
            t,
            url,
            ns,
            "GetVideoSourceConfigurationOptions",
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
        )
        .await;
        let parsed = parse_soap_body(&xml).unwrap();
        let options = parsed.children[0].child("Options").unwrap();
        assert_eq!(
            options
                .path(&["BoundsRange", "WidthRange", "Max"])
                .unwrap()
                .text(),
            "2592"
        );
        assert_eq!(
            options
                .path(&["BoundsRange", "HeightRange", "Max"])
                .unwrap()
                .text(),
            "1944"
        );
        for fields in ["", "<m:ProfileToken>Profile_1</m:ProfileToken>"] {
            let xml = post(t, url, ns, "GetVideoSourceConfigurationOptions", fields).await;
            let parsed = parse_soap_body(&xml).unwrap();
            let opts = parsed.children[0].child("Options").unwrap();
            assert_eq!(
                opts.children_named("VideoSourceTokensAvailable")
                    .map(|c| c.text())
                    .collect::<Vec<_>>(),
                ["VS_1", "VS_2"]
            );
            assert_eq!(
                opts.path(&["BoundsRange", "WidthRange", "Max"])
                    .unwrap()
                    .text(),
                "1280"
            );
            assert_eq!(
                opts.path(&["BoundsRange", "HeightRange", "Max"])
                    .unwrap()
                    .text(),
                "720"
            );
            assert_eq!(opts.attr("MaximumNumberOfProfiles"), Some("8"));
        }
        for (fields, leaf, reason) in [
            (
                "<m:ConfigurationToken>unknown-934</m:ConfigurationToken>",
                "ter:NoConfig",
                "Source configuration not found: unknown-934",
            ),
            (
                "<m:ProfileToken>unknown-profile-934</m:ProfileToken>",
                "ter:NoProfile",
                "Profile not found: unknown-profile-934",
            ),
        ] {
            let xml = post(t, url, ns, "GetVideoSourceConfigurationOptions", fields).await;
            assert_fault(&xml, "ter:InvalidArgVal", Some(leaf), reason);
        }
        for fields in [
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken><m:ConfigurationToken>VSC_2</m:ConfigurationToken>",
            "<m:ConfigurationToken><m:ConfigurationToken>VSC_1</m:ConfigurationToken></m:ConfigurationToken>",
        ] {
            let xml = post(t, url, ns, "GetVideoSourceConfigurationOptions", fields).await;
            assert_fault(&xml, "ter:InvalidArgs", None, "Invalid Args");
        }
        for selector in ["ConfigurationToken", "ProfileToken"] {
            let fields = format!("<m:{selector}>{}</m:{selector}>", "北".repeat(65));
            let xml = post(t, url, ns, "GetVideoSourceConfigurationOptions", &fields).await;
            assert_fault(&xml, "ter:InvalidArgs", None, "Invalid Args");
        }
        let xml = post(
            t,
            url,
            ns,
            "GetVideoSourceConfigurationOptions",
            "<m:ConfigurationToken xmlns:m='urn:test:decoy'>VSC_1</m:ConfigurationToken>",
        )
        .await;
        assert_fault(&xml, "ter:TagMismatch", None, "Tag Mismatch");
    }
    let xml = post(t, url, M2, "GetVideoSourceConfigurations", "<m:ConfigurationToken>VSC_2</m:ConfigurationToken><m:ProfileToken>Profile_1</m:ProfileToken>").await;
    let parsed = parse_soap_body(&xml).unwrap();
    assert_eq!(parsed.children[0].children.len(), 1);
    assert_eq!(parsed.children[0].children[0].attr("token"), Some("VSC_2"));
    let xml = post(
        t,
        url,
        M2,
        "GetVideoSourceConfigurations",
        "<m:ConfigurationToken>unknown-935</m:ConfigurationToken>",
    )
    .await;
    assert_fault(
        &xml,
        "ter:InvalidArgVal",
        Some("ter:NoConfig"),
        "Source configuration not found: unknown-935",
    );
    for op in ["GetVideoSources", "GetVideoSourceConfigurations"] {
        let xml = post(
            t,
            url,
            M1,
            op,
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
        )
        .await;
        assert_fault(&xml, "ter:TagMismatch", None, "Tag Mismatch");
    }
}

#[tokio::test]
async fn source_selectors_and_options() {
    selectors(&MockTransport::new(), "http://mock").await;
}

#[cfg(feature = "mock-server")]
#[tokio::test]
async fn http_source_selectors_and_options() {
    let server = oxvif::mock::MockServer::start().await.unwrap();
    selectors(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
    )
    .await;
}

#[cfg(feature = "metamorph")]
fn recordings() -> oxvif::metamorph::FixtureStore {
    let mut store = oxvif::metamorph::FixtureStore::new("source-936");
    for ns in [M1, M2] {
        for (op, fields) in [
            ("GetVideoSourceConfigurations", ""),
            ("GetProfiles", ""),
            (
                "GetVideoSourceConfigurationOptions",
                "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
            ),
        ] {
            store.record(
                &format!("{ns}/{op}"),
                &envelope(ns, op, fields),
                "<recorded-source-936/>",
            );
        }
    }
    store.record(
        &format!("{M1}/GetVideoSources"),
        &envelope(M1, "GetVideoSources", ""),
        "<recorded-physical-937/>",
    );
    store
}

#[tokio::test]
async fn source_identity_roundtrip() {
    const CONFIG: &str = " cfg &amp; <北>\r\n\t end ";
    const WIRE_CONFIG: &str = " cfg &amp;amp; &lt;北&gt;&#13;&#10;&#9; end ";
    const SOURCE: &str = " sensor & <南>\r\n\t end ";
    const WIRE_SOURCE: &str = " sensor &amp; &lt;南&gt;&#13;&#10;&#9; end ";
    let state = MockState::new();
    state.modify(|s| {
        s.video_sources[0].token = SOURCE.into();
        s.video_source_configs[0].token = CONFIG.into();
        s.video_source_configs[0].source_token = SOURCE.into();
        for p in &mut s.profiles.profiles {
            if p.video_source_config_token.as_deref() == Some("VSC_1") {
                p.video_source_config_token = Some(CONFIG.into());
            }
        }
    });
    async fn check(t: &dyn Transport, url: &str, state: &MockState) {
        let selector = format!("<m:ConfigurationToken>{WIRE_CONFIG}</m:ConfigurationToken>");
        let configured = config("identity-938", "800", "600")
            .replace("VSC_1", WIRE_CONFIG)
            .replace("VS_1", WIRE_SOURCE);
        for ns in [M1, M2] {
            let persistence = if ns == M1 {
                "<m:ForcePersistence>true</m:ForcePersistence>"
            } else {
                ""
            };
            let xml = post(
                t,
                url,
                ns,
                "SetVideoSourceConfiguration",
                &format!("{configured}{persistence}"),
            )
            .await;
            assert_eq!(
                parse_soap_body(&xml).unwrap().children[0].local_name,
                "SetVideoSourceConfigurationResponse"
            );
            let xml = post(
                t,
                url,
                ns,
                if ns == M1 {
                    "GetVideoSourceConfiguration"
                } else {
                    "GetVideoSourceConfigurations"
                },
                &selector,
            )
            .await;
            assert!(xml.contains(&format!("token=\"{WIRE_CONFIG}\"")), "{xml}");
            assert!(
                xml.contains(&format!("<tt:SourceToken>{WIRE_SOURCE}</tt:SourceToken>")),
                "{xml}"
            );
            let xml = post(
                t,
                url,
                ns,
                "GetProfiles",
                if ns == M2 { "<m:Type>All</m:Type>" } else { "" },
            )
            .await;
            assert!(xml.contains(&format!("token=\"{WIRE_CONFIG}\"")), "{xml}");
            assert!(
                xml.contains(&format!("<tt:SourceToken>{WIRE_SOURCE}</tt:SourceToken>")),
                "{xml}"
            );
        }
        let xml = post(t, url, M1, "GetVideoSources", "").await;
        assert!(xml.contains(&format!("token=\"{WIRE_SOURCE}\"")), "{xml}");
        let snapshot = state.read();
        assert_eq!(snapshot.video_source_configs[0].token, CONFIG);
        assert_eq!(snapshot.video_source_configs[0].source_token, SOURCE);
        assert_eq!(snapshot.video_source_configs[0].name, "identity-938");
        assert_eq!(
            (
                snapshot.video_source_configs[0].width,
                snapshot.video_source_configs[0].height
            ),
            (800, 600)
        );
    }
    let t = MockTransport::with_state(state);
    check(&t, "http://mock", t.device()).await;
    #[cfg(feature = "mock-server")]
    {
        let initial = t.device().read().clone();
        let server = oxvif::mock::MockServer::builder()
            .initial_state(initial)
            .start()
            .await
            .unwrap();
        check(
            &oxvif::transport::HttpTransport::default(),
            server.device_url(),
            server.device(),
        )
        .await;
    }
}

#[cfg(feature = "metamorph")]
async fn replay(t: &dyn Transport, url: &str) {
    let xml = post(
        t,
        url,
        M2,
        "SetVideoSourceConfiguration",
        &config("rejected-936", "640", "bad-936"),
    )
    .await;
    assert_fault(
        &xml,
        "ter:InvalidArgVal",
        Some("ter:ConfigModify"),
        "Invalid mock source setting: Bounds/@height",
    );
    for ns in [M1, M2] {
        for (op, fields) in [
            ("GetVideoSourceConfigurations", ""),
            ("GetProfiles", ""),
            (
                "GetVideoSourceConfigurationOptions",
                "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
            ),
        ] {
            assert_eq!(post(t, url, ns, op, fields).await, "<recorded-source-936/>");
        }
    }
    let xml = post(
        t,
        url,
        M1,
        "SetVideoSourceConfiguration",
        &format!(
            "{}<m:ForcePersistence>true</m:ForcePersistence>",
            config("committed-936", "800", "600")
        ),
    )
    .await;
    assert_eq!(
        parse_soap_body(&xml).unwrap().children[0].local_name,
        "SetVideoSourceConfigurationResponse"
    );
    for ns in [M1, M2] {
        let xml = post(t, url, ns, "GetVideoSourceConfigurations", "").await;
        let parsed = parse_soap_body(&xml).unwrap();
        assert_eq!(
            parsed.children[0].children[0].child("Name").unwrap().text(),
            "committed-936"
        );
        let xml = post(
            t,
            url,
            ns,
            "GetVideoSourceConfigurationOptions",
            "<m:ConfigurationToken>VSC_1</m:ConfigurationToken>",
        )
        .await;
        let parsed = parse_soap_body(&xml).unwrap();
        assert_eq!(
            parsed.children[0]
                .path(&["Options", "BoundsRange", "WidthRange", "Max"])
                .unwrap()
                .text(),
            "2592"
        );
        let xml = post(t, url, ns, "GetProfiles", "").await;
        assert_eq!(
            parse_soap_body(&xml).unwrap().children[0].local_name,
            "GetProfilesResponse"
        );
    }
    assert_eq!(
        post(t, url, M1, "GetVideoSources", "").await,
        "<recorded-physical-937/>"
    );
}

#[cfg(feature = "metamorph")]
#[tokio::test]
async fn source_replay_commit_boundary() {
    replay(
        &oxvif::metamorph::MetamorphTransport::new(recordings()),
        "http://mock",
    )
    .await;
}

#[cfg(feature = "metamorph-server")]
#[tokio::test]
async fn http_source_replay_commit_boundary() {
    let server = oxvif::mock::MockServer::builder()
        .replay(recordings())
        .start()
        .await
        .unwrap();
    replay(
        &oxvif::transport::HttpTransport::default(),
        server.device_url(),
    )
    .await;
}
