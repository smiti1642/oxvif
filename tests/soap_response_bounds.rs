//! Project-authored syntax/resource probes, not schema-derived conformance cases.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use oxvif::{
    OnvifClient, OnvifError,
    soap::{
        MAX_XML_BYTES, MAX_XML_DEPTH, MAX_XML_NODES, SoapError, XmlNode, XmlResource,
        parse_soap_body,
    },
    transport::{HttpTransport, HttpTransportOptions, Transport, TransportError},
};

fn envelope(body: &str) -> String {
    format!("<Envelope><Body>{body}</Body></Envelope>")
}

fn deep(depth: usize) -> String {
    format!("{}tok-64{}", "<X>".repeat(depth), "</X>".repeat(depth))
}

fn hostname() -> String {
    envelope(
        "<GetHostnameResponse><HostnameInformation><FromDHCP>false</FromDHCP><Name>camera-17</Name></HostnameInformation></GetHostnameResponse>",
    )
}

#[test]
fn complete_path_rejects_partial_multiple_and_malformed_documents() {
    for (xml, reason) in [
        (
            "<?xml version='1.0' version='1.0'?><Envelope><Body/></Envelope>",
            "invalid XML declaration",
        ),
        (
            "<?xml version='1.0' standalone='maybe'?><Envelope><Body/></Envelope>",
            "invalid XML declaration",
        ),
        (
            "<?xml version='1.1'?><Envelope><Body/></Envelope>",
            "unsupported XML version",
        ),
        (
            "<?xml version='1.0' encoding='iso-8859-1'?><Envelope><Body/></Envelope>",
            "unsupported XML encoding declaration",
        ),
        ("<Envelope><0/></Envelope>", "invalid XML element name"),
        (
            "<Envelope><Body 0='x'/></Envelope>",
            "invalid XML attribute",
        ),
        ("<Envelope><Body>unfinished", "incomplete XML document"),
        ("<Envelope><Body/></Envelope><Other/>", "multiple XML roots"),
        (
            "<Envelope><Body/></Envelope>secret",
            "content outside XML root",
        ),
        (
            "secret<Envelope><Body/></Envelope>",
            "content outside XML root",
        ),
        (
            "<Envelope><Body key='one' key='two'/></Envelope>",
            "invalid XML attribute",
        ),
        (
            "<Envelope><Body key=noquote/></Envelope>",
            "invalid XML attribute",
        ),
        (
            "<Envelope><Body key='&unknown;'/></Envelope>",
            "invalid XML attribute",
        ),
        (
            "<Envelope><Body key='<'/></Envelope>",
            "invalid XML attribute",
        ),
        (
            "<!DOCTYPE Envelope><Envelope><Body/></Envelope>",
            "DTD is not allowed in SOAP XML",
        ),
        (
            "<Envelope><Body>&unknown;</Body></Envelope>",
            "invalid XML reference",
        ),
        (
            "<Envelope><Body>&#x110000;</Body></Envelope>",
            "invalid XML reference",
        ),
        (
            "<Envelope><Body>&#0;</Body></Envelope>",
            "invalid XML reference",
        ),
        (
            "<Envelope><Body>\0</Body></Envelope>",
            "invalid XML character",
        ),
        (
            "<Envelope><Body/></Envelope><![CDATA[secret]]>",
            "CDATA outside XML root",
        ),
        (
            "<Envelope><Body/></Envelope><?xml version='1.0'?>",
            "misplaced XML declaration",
        ),
    ] {
        assert_eq!(
            parse_soap_body(xml).unwrap_err(),
            SoapError::XmlParse(reason.into()),
            "{xml}"
        );
    }
    for xml in [
        "",
        "<Envelope><Body></Envelope>",
        "<Envelope><Body/><!-- bad--comment --></Envelope>",
    ] {
        assert!(matches!(parse_soap_body(xml), Err(SoapError::XmlParse(_))));
    }
    // The public legacy entry point keeps its independently pinned behavior.
    assert_eq!(
        XmlNode::parse("<A>unfinished").unwrap().text(),
        "unfinished"
    );
    assert_eq!(XmlNode::parse("<A/><B/>").unwrap().local_name, "A");
    assert_eq!(
        XmlNode::parse("<A key='&unknown;'/>").unwrap().attr("key"),
        Some("")
    );
}

#[test]
fn complete_path_preserves_unicode_entities_cdata_attributes_and_trailing_misc() {
    let xml = "<?xml version='1.0' encoding='UTF-8'?><s:Envelope><s:Body><n:攝影機 xmlns:n='urn:synthetic' n:名稱=\"前門\tA\r\nB&#xA;&#x9;&amp;\">  攝影機 &amp; &#x1F4F7;<![CDATA[ <raw>&text\r\n]]> tail\r\nend  </n:攝影機></s:Body></s:Envelope>\r\n<!-- after --><?after data?>";
    let body = parse_soap_body(xml).unwrap();
    let node = body.child("攝影機").unwrap();
    assert_eq!(node.attr("名稱"), Some("前門 A B\n\t&"));
    assert_eq!(node.text(), "攝影機 & 📷 <raw>&text\r\n tail\nend");
    assert_eq!(node.attrs.len(), 1);
}

#[test]
fn wide_attributes_are_validated_before_the_compatibility_dom_is_built() {
    let attributes = (0..4096)
        .map(|index| format!(" k{index}='{index}'"))
        .collect::<String>();
    let node = XmlNode::parse_complete(&format!("<Wide{attributes}/>")).unwrap();
    assert_eq!(node.attrs.len(), 4096);
    assert_eq!(node.attr("k0"), Some("0"));
    assert_eq!(node.attr("k4095"), Some("4095"));
    assert_eq!(
        XmlNode::parse_complete(&format!("<Wide{attributes} k0='other'/>")).unwrap_err(),
        SoapError::XmlParse("invalid XML attribute".into())
    );
}

#[test]
fn exact_production_limits_count_markup_empty_elements_and_root_depth() {
    let shell = envelope("<X></X>");
    let payload_len = MAX_XML_BYTES - shell.len();
    let exact = envelope(&format!("<X>{}</X>", "x".repeat(payload_len)));
    assert_eq!(exact.len(), MAX_XML_BYTES);
    let body = parse_soap_body(&exact).unwrap();
    let text = body.child("X").unwrap().text();
    assert_eq!(text.len(), payload_len);
    assert!(text.bytes().all(|byte| byte == b'x'));
    assert_eq!(
        parse_soap_body(&(exact + " ")).unwrap_err(),
        SoapError::ResourceLimit {
            resource: XmlResource::Bytes,
            limit: MAX_XML_BYTES
        }
    );

    let body = parse_soap_body(&envelope(&deep(MAX_XML_DEPTH - 2))).unwrap();
    let mut leaf = &body;
    for _ in 0..MAX_XML_DEPTH - 2 {
        leaf = leaf.child("X").unwrap();
    }
    assert_eq!(leaf.text(), "tok-64");
    assert_eq!(
        parse_soap_body(&envelope(&deep(MAX_XML_DEPTH - 1))).unwrap_err(),
        SoapError::ResourceLimit {
            resource: XmlResource::Depth,
            limit: MAX_XML_DEPTH
        }
    );

    let body = parse_soap_body(&envelope(&"<Item/>".repeat(MAX_XML_NODES - 2))).unwrap();
    assert_eq!(body.children.len(), MAX_XML_NODES - 2);
    assert!(
        body.children
            .iter()
            .all(|node| node.local_name == "Item" && node.text.is_none())
    );
    assert_eq!(
        parse_soap_body(&envelope(&"<Item/>".repeat(MAX_XML_NODES - 1))).unwrap_err(),
        SoapError::ResourceLimit {
            resource: XmlResource::Nodes,
            limit: MAX_XML_NODES
        }
    );
}

struct Responses(Mutex<VecDeque<String>>);

#[async_trait::async_trait]
impl Transport for Responses {
    async fn soap_post(&self, _: &str, action: &str, _: String) -> Result<String, TransportError> {
        assert_eq!(action, "http://www.onvif.org/ver10/device/wsdl/GetHostname");
        Ok(self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected extra request"))
    }
}

#[tokio::test]
async fn injected_transport_is_bounded_and_a_valid_followup_recovers() {
    for (bad, expected) in [
        (
            "<Envelope><Body>unfinished".into(),
            SoapError::XmlParse("incomplete XML document".into()),
        ),
        (
            envelope(&deep(MAX_XML_DEPTH - 1)),
            SoapError::ResourceLimit {
                resource: XmlResource::Depth,
                limit: MAX_XML_DEPTH,
            },
        ),
        (
            "x".repeat(MAX_XML_BYTES + 1),
            SoapError::ResourceLimit {
                resource: XmlResource::Bytes,
                limit: MAX_XML_BYTES,
            },
        ),
    ] {
        let responses = Arc::new(Responses(Mutex::new(VecDeque::from([bad, hostname()]))));
        let client = OnvifClient::new("http://synthetic").with_transport(responses.clone());
        match client.get_hostname().await.unwrap_err() {
            OnvifError::Soap(actual) => assert_eq!(actual, expected),
            other => panic!("unexpected error: {other}"),
        }
        let host = client.get_hostname().await.unwrap();
        assert_eq!(host.name.as_deref(), Some("camera-17"));
        assert!(!host.from_dhcp);
        assert!(responses.0.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn network_and_injected_responses_reach_the_same_parser_limits() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (bad, resource, limit) in [
        (
            envelope(&deep(MAX_XML_DEPTH - 1)),
            XmlResource::Depth,
            MAX_XML_DEPTH,
        ),
        (
            "x".repeat(MAX_XML_BYTES + 1),
            XmlResource::Bytes,
            MAX_XML_BYTES,
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for response in [bad, hostname()] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&buffer[..count]);
                    if request.ends_with(b"</s:Envelope>") {
                        break;
                    }
                    assert!(request.len() < 65_536);
                }
                assert!(String::from_utf8(request).unwrap().contains("GetHostname"));
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            response.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        });
        // Raise the network cap explicitly to isolate the production parse cap.
        let transport = HttpTransport::from_options(HttpTransportOptions {
            max_response_bytes: MAX_XML_BYTES + 1024,
            ..Default::default()
        })
        .unwrap();
        let client = OnvifClient::new(format!("http://{address}/onvif/device_service"))
            .with_transport(Arc::new(transport));
        match client.get_hostname().await.unwrap_err() {
            OnvifError::Soap(actual) => {
                assert_eq!(actual, SoapError::ResourceLimit { resource, limit })
            }
            other => panic!("unexpected error: {other}"),
        }
        assert_eq!(
            client.get_hostname().await.unwrap().name.as_deref(),
            Some("camera-17")
        );
        server.await.unwrap();
    }
}
