use super::*;

#[test]
fn independent_verifier_refuses_tampered_digest_fields() {
    let mut p = prompt(r#"Digest realm="r", nonce="n", qop="auth""#);
    let ctx = AuthContext::new("u", "p", "/image?channel=2");
    let a = p.respond(&ctx).unwrap();
    assert!(verify(&a, "p", "r", "n", "auth", "MD5"));
    assert!(!verify(&a, "wrong", "r", "n", "auth", "MD5"));
    assert!(!verify(&a, "p", "other-realm", "n", "auth", "MD5"));
    assert!(!verify(&a, "p", "r", "other-nonce", "auth", "MD5"));
    let mut corrupt = a;
    corrupt.response = "00000000000000000000000000000000".into();
    assert!(!verify(&corrupt, "p", "r", "n", "auth", "MD5"));
}

#[tokio::test]
async fn unsupported_digest_never_sends_basic_and_stale_cannot_change_realm() {
    for changed_realm in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/image?channel=2", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut count = 0;
            for i in 0..if changed_realm { 2 } else { 1 } {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_request(&mut socket).await;
                assert!(
                    !request
                        .to_ascii_lowercase()
                        .contains("authorization: basic")
                );
                let value = if !changed_realm {
                    "WWW-Authenticate: Digest realm=\"r\", nonce=\"n\", algorithm=unsupported, Basic realm=\"r\"\r\n"
                } else if i == 0 {
                    "WWW-Authenticate: Digest realm=\"r\", nonce=\"n\", qop=\"auth\"\r\n"
                } else {
                    "WWW-Authenticate: Digest realm=\"changed\", nonce=\"n2\", qop=\"auth\", stale=true\r\n"
                };
                send(&mut socket, 401, value, b"").await;
                count += 1;
            }
            count
        });
        let e = fetch(&url, &url, Some(("u", "p")), &SnapshotOptions::default())
            .await
            .unwrap_err();
        assert_eq!(
            e.to_string(),
            if changed_realm {
                "Snapshot returned HTTP 401; redirects are not followed."
            } else {
                "Unsupported snapshot authentication challenge."
            }
        );
        assert_eq!(server.await.unwrap(), if changed_realm { 2 } else { 1 });
    }
}

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

// A synthetic 1x1 red JPEG, generated and independently decoded with System.Drawing.
// No real-camera image is embedded. Signature checking still is not full decoding.
const JPEG: &[u8] = &[
    255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 1, 0, 96, 0, 96, 0, 0, 255, 219, 0, 67, 0,
    8, 6, 6, 7, 6, 5, 8, 7, 7, 7, 9, 9, 8, 10, 12, 20, 13, 12, 11, 11, 12, 25, 18, 19, 15, 20, 29,
    26, 31, 30, 29, 26, 28, 28, 32, 36, 46, 39, 32, 34, 44, 35, 28, 28, 40, 55, 41, 44, 48, 49, 52,
    52, 52, 31, 39, 57, 61, 56, 50, 60, 46, 51, 52, 50, 255, 219, 0, 67, 1, 9, 9, 9, 12, 11, 12,
    24, 13, 13, 24, 50, 33, 28, 33, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50,
    50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 50,
    50, 50, 50, 50, 50, 50, 50, 50, 50, 50, 255, 192, 0, 17, 8, 0, 1, 0, 1, 3, 1, 34, 0, 2, 17, 1,
    3, 17, 1, 255, 196, 0, 31, 0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5,
    6, 7, 8, 9, 10, 11, 255, 196, 0, 181, 16, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125, 1,
    2, 3, 0, 4, 17, 5, 18, 33, 49, 65, 6, 19, 81, 97, 7, 34, 113, 20, 50, 129, 145, 161, 8, 35, 66,
    177, 193, 21, 82, 209, 240, 36, 51, 98, 114, 130, 9, 10, 22, 23, 24, 25, 26, 37, 38, 39, 40,
    41, 42, 52, 53, 54, 55, 56, 57, 58, 67, 68, 69, 70, 71, 72, 73, 74, 83, 84, 85, 86, 87, 88, 89,
    90, 99, 100, 101, 102, 103, 104, 105, 106, 115, 116, 117, 118, 119, 120, 121, 122, 131, 132,
    133, 134, 135, 136, 137, 138, 146, 147, 148, 149, 150, 151, 152, 153, 154, 162, 163, 164, 165,
    166, 167, 168, 169, 170, 178, 179, 180, 181, 182, 183, 184, 185, 186, 194, 195, 196, 197, 198,
    199, 200, 201, 202, 210, 211, 212, 213, 214, 215, 216, 217, 218, 225, 226, 227, 228, 229, 230,
    231, 232, 233, 234, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 255, 196, 0, 31, 1, 0, 3,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 255, 196, 0,
    181, 17, 0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119, 0, 1, 2, 3, 17, 4, 5, 33, 49, 6, 18,
    65, 81, 7, 97, 113, 19, 34, 50, 129, 8, 20, 66, 145, 161, 177, 193, 9, 35, 51, 82, 240, 21, 98,
    114, 209, 10, 22, 36, 52, 225, 37, 241, 23, 24, 25, 26, 38, 39, 40, 41, 42, 53, 54, 55, 56, 57,
    58, 67, 68, 69, 70, 71, 72, 73, 74, 83, 84, 85, 86, 87, 88, 89, 90, 99, 100, 101, 102, 103,
    104, 105, 106, 115, 116, 117, 118, 119, 120, 121, 122, 130, 131, 132, 133, 134, 135, 136, 137,
    138, 146, 147, 148, 149, 150, 151, 152, 153, 154, 162, 163, 164, 165, 166, 167, 168, 169, 170,
    178, 179, 180, 181, 182, 183, 184, 185, 186, 194, 195, 196, 197, 198, 199, 200, 201, 202, 210,
    211, 212, 213, 214, 215, 216, 217, 218, 226, 227, 228, 229, 230, 231, 232, 233, 234, 242, 243,
    244, 245, 246, 247, 248, 249, 250, 255, 218, 0, 12, 3, 1, 0, 2, 17, 3, 17, 0, 63, 0, 226, 232,
    162, 138, 249, 147, 247, 19, 255, 217,
];

fn headers(value: &str) -> header::HeaderMap {
    let mut h = header::HeaderMap::new();
    h.insert(header::WWW_AUTHENTICATE, value.parse().unwrap());
    h
}
fn prompt(value: &str) -> WwwAuthenticateHeader {
    match select_challenge(&headers(value)).expect("supported fixture") {
        Challenge::Digest(p) => p,
        Challenge::Basic => panic!("expected digest"),
    }
}

#[test]
fn digest_challenge_case_lists_unknown_options_and_escaping() {
    let mut p = prompt(
        r#"Basic realm="basic", dIgEsT ReAlM="fixture,\"quoted\"", Nonce="n", QOP="unknown,auth", ALGORITHM=SHA-256"#,
    );
    assert_eq!(p.realm, "fixture,\"quoted\"");
    assert_eq!(p.algorithm.to_string(), "SHA-256");
    assert_eq!(p.qop, Some(vec![Qop::AUTH]));
    let answer = p.respond(&AuthContext::new("u", "p", "/image")).unwrap();
    assert_eq!(answer.realm, "fixture,\"quoted\"");
    let p = prompt(
        r#"Digest realm="r", nonce="n", algorithm=unsupported, Basic realm="b", Digest realm="chosen", nonce="fresh", algorithm=MD5"#,
    );
    assert_eq!(p.realm, "chosen");
    let mut h = headers(r#"Digest realm="r", nonce="n", algorithm=unsupported"#);
    h.append(
        header::WWW_AUTHENTICATE,
        r#"Digest realm="second", nonce="n", algorithm=SHA-256"#
            .parse()
            .unwrap(),
    );
    assert!(matches!(select_challenge(&h), Ok(Challenge::Digest(p)) if p.realm == "second"));
    for input in [
        r#"Digest realm="r", Realm="duplicate", nonce="n", Basic realm="b""#,
        r#"Digest realm="r", nonce="n", qop="unknown", Basic realm="b""#,
        r#"Digest realm="unterminated, nonce="n", Basic realm="b""#,
        r#"Digest realm="r", nonce="n", algorithm=MD5-sess, Basic realm="b""#,
        r#"Bearer opaque"#,
    ] {
        let error = select_challenge(&headers(input))
            .err()
            .expect("must refuse");
        assert_eq!(
            error.to_string(),
            "Unsupported snapshot authentication challenge."
        );
        assert!(!error.is_retryable());
    }
}

#[test]
fn digest_rfc_vector_and_empty_get_auth_int_are_not_self_comparisons() {
    let mut p = prompt(
        r#"Digest realm="testrealm@host.com", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", qop="auth", opaque="5ccc069c403ebaf9f0171e9517f40e41""#,
    );
    let mut ctx = AuthContext::new("Mufasa", "Circle Of Life", "/dir/index.html");
    ctx.set_custom_cnonce("0a4f113b");
    let a = p.respond(&ctx).unwrap();
    assert_eq!(a.response, "6629fae49393a05397450978507c4ef1");
    assert!(a.to_header_string().contains("qop=auth, nc=00000001"));
    assert!(!a.to_header_string().contains("qop=\"auth\""));
    let mut p = prompt(r#"Digest realm="r", nonce="n", qop="auth-int""#);
    let ctx =
        AuthContext::new_with_method("u", "p", "/image?channel=2", Some(&[][..]), HttpMethod::GET);
    let a = p.respond(&ctx).unwrap();
    assert_eq!(a.qop, Some(Qop::AUTH_INT));
    assert!(verify(&a, "p", "r", "n", "auth-int", "MD5"));
}

// The mock authenticates before returning a JPEG. It does NOT call respond/digest
// to calculate its expectation, and does not use the production challenge parser.
fn verify(
    a: &digest_auth::AuthorizationHeader,
    password: &str,
    realm: &str,
    nonce: &str,
    qop: &str,
    algorithm: &str,
) -> bool {
    let hash = algorithm.parse::<Algorithm>().unwrap();
    let cnonce = a.cnonce.as_deref().unwrap_or("");
    let ha1 = hash.hash_str(&format!("u:{realm}:{password}"));
    let ha1 = if hash.sess {
        hash.hash_str(&format!("{ha1}:{nonce}:{cnonce}"))
    } else {
        ha1
    };
    let a2 = if qop == "auth-int" {
        format!("GET:/image?channel=2:{}", hash.hash(b""))
    } else {
        "GET:/image?channel=2".into()
    };
    let expected = hash.hash_str(&format!(
        "{ha1}:{nonce}:00000001:{cnonce}:{qop}:{}",
        hash.hash_str(&a2)
    ));
    a.username == "u"
        && a.realm == realm
        && a.nonce == nonce
        && a.uri == "/image?channel=2"
        && a.nc == 1
        && a.qop.is_some_and(|q| q.to_string() == qop)
        && a.algorithm == hash
        && a.response == expected
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut result = Vec::new();
    loop {
        let mut buf = [0; 1024];
        let n = socket.read(&mut buf).await.unwrap();
        assert!(n > 0 && result.len() < 65536);
        result.extend_from_slice(&buf[..n]);
        if result.windows(4).any(|b| b == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(result).unwrap()
}
async fn send(socket: &mut tokio::net::TcpStream, status: u16, challenge: &str, body: &[u8]) {
    let headers = format!(
        "HTTP/1.1 {status} Test\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: image/jpeg\r\n{challenge}\r\n",
        body.len()
    );
    socket.write_all(headers.as_bytes()).await.unwrap();
    socket.write_all(body).await.unwrap();
}
async fn authenticated_server(
    qop: &'static str,
    algorithm: &'static str,
    stale: bool,
    reject_final: bool,
) -> (String, tokio::task::JoinHandle<Vec<bool>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/image?channel=2", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut verified = Vec::new();
            for i in 0..if stale {3} else {2} {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_request(&mut socket).await;
                assert!(request.starts_with("GET /image?channel=2 HTTP/1.1\r\n"));
                let valid = if i == 0 {
                    assert!(!request.to_ascii_lowercase().contains("authorization:"));
                    false
                } else {
                    // This intentionally models the known firmware quirk. The
                    // separate hash validation still enforces real authentication.
                    let auth = request.lines().find_map(|l| l.strip_prefix("Authorization: ")).unwrap_or("");
                    let a = digest_auth::AuthorizationHeader::parse(auth);
                    let nonce = if i == 1 {"n1"} else {"n2"};
                    let valid = a.ok().is_some_and(|a| verify(&a, "correct", "fixture", nonce, qop, algorithm));
                    verified.push(valid);
                    valid
                };
                if valid && !(i == 1 && stale) && !reject_final {
                    send(&mut socket, 200, "", JPEG).await;
                } else {
                    let nonce = if i == 0 {"n1"} else if i == 1 {"n2"} else {"n3"};
                    let challenge = format!("WWW-Authenticate: Digest realm=\"fixture\", nonce=\"{nonce}\", qop=\"{qop}\", algorithm={algorithm}, stale={}\r\nWWW-Authenticate: Basic realm=\"fixture\"\r\n", stale && i > 0);
                    send(&mut socket, 401, &challenge, b"").await;
                }
            }
            verified
        }).await.expect("server deadline")
    });
    (url, task)
}

#[tokio::test]
async fn shared_cli_and_health_path_require_valid_digest_before_image_success() {
    for (qop, algorithm, health) in [
        ("auth", "MD5", false),
        ("auth-int", "MD5", false),
        ("auth", "SHA-256", false),
        ("auth", "MD5-sess", false),
        ("auth", "MD5", true),
    ] {
        let (url, server) = authenticated_server(qop, algorithm, false, false).await;
        if health {
            let bytes = crate::health::checks::fetch_snapshot(
                &url,
                &url,
                Some(&("u".into(), "correct".into())),
                &SnapshotOptions::default(),
            )
            .await
            .unwrap();
            assert_eq!(bytes, JPEG.len());
        } else {
            let result = fetch(
                &url,
                &url,
                Some(("u", "correct")),
                &SnapshotOptions::default(),
            )
            .await;
            assert!(
                result.is_ok(),
                "valid Digest fixture was refused: {result:?}"
            );
            let (bytes, kind) = result.unwrap();
            assert_eq!(bytes, JPEG);
            assert_eq!(kind, "jpeg");
        }
        assert_eq!(
            server.await.unwrap(),
            vec![true],
            "server must validate the actual digest"
        );
    }
}

#[tokio::test]
async fn wrong_password_is_not_success_and_never_falls_back_to_basic() {
    let (url, server) = authenticated_server("auth", "MD5", false, false).await;
    let e = fetch(
        &url,
        &url,
        Some(("u", "wrong")),
        &SnapshotOptions::default(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Snapshot returned HTTP 401; redirects are not followed."
    );
    assert!(!e.is_retryable());
    assert_eq!(server.await.unwrap(), vec![false]);
}

#[tokio::test]
async fn stale_nonce_retry_is_bounded_and_uses_the_new_nonce() {
    for reject in [false, true] {
        let (url, server) = authenticated_server("auth", "MD5", true, reject).await;
        let result = fetch(
            &url,
            &url,
            Some(("u", "correct")),
            &SnapshotOptions::default(),
        )
        .await;
        if reject {
            assert_eq!(
                result.unwrap_err().to_string(),
                "Snapshot returned HTTP 401; redirects are not followed."
            );
        } else {
            assert_eq!(result.unwrap().0, JPEG);
        }
        assert_eq!(server.await.unwrap(), vec![true, true]);
    }
}

#[test]
fn destination_and_image_claims_are_explicit() {
    for uri in [
        "http://foreign.test/s",
        "http://u:p@camera.test/s",
        "https://camera.test/s#x",
        "file:///x",
        "http://camera.test/s",
    ] {
        let e = validate_url(uri, "https://camera.test/onvif").unwrap_err();
        assert!(e.is_invalid_argument());
        assert_eq!(
            e.to_string(),
            "Snapshot URL rejected: require HTTP(S), the device host, no userinfo/fragment and no HTTPS downgrade."
        );
    }
    assert_eq!(
        validate_url("https://camera.test:8443/s", "https://camera.test/onvif")
            .unwrap()
            .port(),
        Some(8443)
    );
    assert_eq!(image_type(JPEG), Some("jpeg"));
    assert_eq!(image_type(&JPEG[..JPEG.len() - 1]), None);
    assert_eq!(image_type(b"<html>login</html>"), None);
}
