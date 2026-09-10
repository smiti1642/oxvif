//! Shared-state consistency probes using project-authored generation markers.
#![cfg(feature = "mock")]

use oxvif::{
    mock::MockTransport,
    soap::{find_response, parse_soap_body},
    transport::Transport,
};
use std::sync::{Arc, mpsc};

#[tokio::test]
async fn profile_reads_do_not_mix_profile_and_configuration_generations() {
    let transport = Arc::new(MockTransport::new());
    transport.device().modify(|state| {
        state.profiles.profiles[0].name = "generation-0".into();
        state.video_source_configs[0].name = "generation-0".into();
    });
    let writer_transport = transport.clone();
    let (start, work) = mpsc::channel::<usize>();
    let (ready, started) = mpsc::channel();
    let (complete, completed) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        while let Ok(generation) = work.recv() {
            for step in 0..256 {
                writer_transport.device().modify(|state| {
                    let value = format!("generation-{generation}-{step}");
                    state.profiles.profiles[0].name = value.clone();
                    state.video_source_configs[0].name = value;
                });
                if step == 0 && ready.send(()).is_err() {
                    return;
                }
                std::thread::yield_now();
            }
            if complete.send(()).is_err() {
                break;
            }
        }
    });
    let mut mismatches = Vec::new();
    let mut mixed_by_path = std::collections::BTreeMap::new();
    let mut generation = 0;
    for (namespace, operation, fields, response_name, profile_name, config_path) in [
        (
            "http://www.onvif.org/ver10/media/wsdl",
            "GetProfiles",
            "",
            "GetProfilesResponse",
            "Profiles",
            &["VideoSourceConfiguration"][..],
        ),
        (
            "http://www.onvif.org/ver10/media/wsdl",
            "GetProfile",
            "<m:ProfileToken>Profile_1</m:ProfileToken>",
            "GetProfileResponse",
            "Profile",
            &["VideoSourceConfiguration"][..],
        ),
        (
            "http://www.onvif.org/ver20/media/wsdl",
            "GetProfiles",
            "<m:Token>Profile_1</m:Token><m:Type>All</m:Type>",
            "GetProfilesResponse",
            "Profiles",
            &["Configurations", "VideoSource"][..],
        ),
    ] {
        for _ in 0..400 {
            generation += 1;
            start.send(generation).unwrap();
            started.recv().unwrap();
            let xml = transport.soap_post("http://mock", &format!("{namespace}/{operation}"), format!(
                "<s:Envelope xmlns:s='http://www.w3.org/2003/05/soap-envelope' xmlns:m='{namespace}'><s:Body><m:{operation}>{fields}</m:{operation}></s:Body></s:Envelope>"
            )).await.unwrap();
            // Every handoff completes before the next request; no sleeps or
            // unbounded background mutation continues after this test finishes.
            completed.recv().unwrap();
            let body = parse_soap_body(&xml).unwrap();
            let profile = find_response(&body, response_name)
                .unwrap()
                .child(profile_name)
                .unwrap();
            assert_eq!(profile.attr("token"), Some("Profile_1"));
            let config = profile.path(config_path).unwrap();
            assert_eq!(config.attr("token"), Some("VSC_1"));
            let profile_generation = profile.child("Name").unwrap().text();
            let config_generation = config.child("Name").unwrap().text();
            assert!(profile_generation.starts_with("generation-"));
            if profile_generation != config_generation {
                *mixed_by_path
                    .entry(format!("{namespace}/{operation}"))
                    .or_insert(0) += 1;
                if mismatches.len() < 12 {
                    mismatches.push(format!(
                        "{namespace}/{operation}: {profile_generation} != {config_generation}"
                    ));
                }
            }
        }
    }
    drop(start);
    writer.join().unwrap();
    assert_eq!(
        transport.device().read().profiles.profiles[0].name,
        format!("generation-{generation}-255")
    );
    assert_eq!(
        transport.device().read().video_source_configs[0].name,
        format!("generation-{generation}-255")
    );
    assert!(
        mixed_by_path.is_empty(),
        "mixed snapshots by path: {mixed_by_path:?}; examples: {mismatches:?}"
    );
}
