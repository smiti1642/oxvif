//! Bounded HTTP policy, separate from SOAP routing and explicit raw fixtures.

use axum::http::{HeaderMap, StatusCode, header};

use super::request::Request;

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const MAX_CONTENT_TYPE: usize = 8192;

/// Header failures happen before entering the SOAP pipeline; never echo input.
pub(super) fn action(headers: &HeaderMap) -> Result<String, StatusCode> {
    let bad = StatusCode::BAD_REQUEST;
    let unsupported = StatusCode::UNSUPPORTED_MEDIA_TYPE;
    let mut types = headers.get_all(header::CONTENT_TYPE).iter();
    let content_type = types.next().ok_or(unsupported)?;
    if types.next().is_some() || content_type.as_bytes().len() > MAX_CONTENT_TYPE {
        return Err(bad);
    }
    let mut codings = headers.get_all(header::CONTENT_ENCODING).iter();
    if let Some(coding) = codings.next() {
        if codings.next().is_some() {
            return Err(bad);
        }
        if !coding
            .to_str()
            .map(|v| v.trim().eq_ignore_ascii_case("identity"))
            .unwrap_or(false)
        {
            return Err(unsupported);
        }
    }
    let value = content_type.to_str().map_err(|_| bad)?;
    let (media, mut rest) = value.split_once(';').unwrap_or((value, ""));
    if !media.trim().eq_ignore_ascii_case("application/soap+xml") {
        return Err(unsupported);
    }
    let mut names = std::collections::HashSet::new();
    let mut action = String::new();
    // A trailing semicolon is malformed, including when no parameter follows it.
    if value.contains(';') && rest.trim().is_empty() {
        return Err(bad);
    }
    while !rest.is_empty() {
        rest = rest.trim_start_matches([' ', '\t']);
        let (name, tail) = rest.split_once('=').ok_or(bad)?;
        let name = name.trim_end_matches([' ', '\t']);
        if name.is_empty() || !name.bytes().all(token) || !names.insert(name.to_ascii_lowercase()) {
            return Err(bad);
        }
        let (parameter, tail) = parameter(tail.trim_start_matches([' ', '\t']))?;
        if name.eq_ignore_ascii_case("charset") && !parameter.eq_ignore_ascii_case("utf-8") {
            return Err(unsupported);
        }
        if name.eq_ignore_ascii_case("action") {
            if !absolute_action(&parameter) {
                return Err(bad);
            }
            action = parameter;
        }
        rest = tail.trim_start_matches([' ', '\t']);
        if !rest.is_empty() {
            rest = rest.strip_prefix(';').ok_or(bad)?;
            if rest.trim().is_empty() {
                return Err(bad);
            }
        }
    }
    Ok(action)
}

fn token(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn parameter(input: &str) -> Result<(String, &str), StatusCode> {
    let bad = StatusCode::BAD_REQUEST;
    if let Some(quoted) = input.strip_prefix('"') {
        let mut decoded = String::new();
        let mut bytes = quoted.bytes().enumerate();
        while let Some((index, b)) = bytes.next() {
            match b {
                b'"' => return Ok((decoded, &quoted[index + 1..])),
                b'\\' => {
                    let (_, escaped) = bytes.next().ok_or(bad)?;
                    if !matches!(escaped, b'\t' | b' '..=b'~') {
                        return Err(bad);
                    }
                    decoded.push(char::from(escaped));
                }
                b'\t' | b' '..=b'~' => decoded.push(char::from(b)),
                _ => return Err(bad),
            }
        }
        Err(bad)
    } else {
        let end = input.bytes().take_while(|b| token(*b)).count();
        if end == 0 {
            return Err(bad);
        }
        Ok((input[..end].to_owned(), &input[end..]))
    }
}

fn absolute_action(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    if !scheme
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic())
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        || rest.is_empty()
    {
        return false;
    }
    let (hierarchy, query) = rest.split_once('?').unwrap_or((rest, ""));
    if !uri_chars(query, b":@/?") {
        return false;
    }
    let path = if let Some(authority_path) = hierarchy.strip_prefix("//") {
        let (authority, path) = authority_path
            .find('/')
            .map(|index| authority_path.split_at(index))
            .unwrap_or((authority_path, ""));
        if !uri_authority(authority) {
            return false;
        }
        path
    } else {
        hierarchy
    };
    uri_chars(path, b":@/")
}

// RFC 3986 component characters; never normalize or resolve the Action value.
fn uri_chars(value: &str, extra: &[u8]) -> bool {
    let mut bytes = value.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            if !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !b.is_ascii_alphanumeric()
            && !b"-._~!$&'()*+,;=".contains(&b)
            && !extra.contains(&b)
        {
            return false;
        }
    }
    true
}

fn uri_authority(authority: &str) -> bool {
    let host_port = if let Some((user, host)) = authority.rsplit_once('@') {
        if !uri_chars(user, b":") {
            return false;
        }
        host
    } else {
        authority
    };
    if let Some(literal) = host_port.strip_prefix('[') {
        let Some((host, tail)) = literal.split_once(']') else {
            return false;
        };
        let valid_host = host.parse::<std::net::Ipv6Addr>().is_ok()
            || host
                .strip_prefix(['v', 'V'])
                .and_then(|future| future.split_once('.'))
                .is_some_and(|(version, address)| {
                    !version.is_empty()
                        && version.bytes().all(|b| b.is_ascii_hexdigit())
                        && !address.is_empty()
                        && address
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:".contains(&b))
                });
        valid_host
            && (tail.is_empty()
                || tail
                    .strip_prefix(':')
                    .is_some_and(|port| port.bytes().all(|b| b.is_ascii_digit())))
    } else {
        let (host, port) = host_port.split_once(':').unwrap_or((host_port, ""));
        uri_chars(host, b"") && port.bytes().all(|b| b.is_ascii_digit())
    }
}

/// Recognized SOAP Fault codes only: malformed/raw fixtures remain untouched.
pub(super) fn response_status(xml: &str) -> StatusCode {
    let code = || {
        // Validate lexical syntax without constructing a second DOM. The scoped
        // request parser below applies its own tighter inspection limits.
        crate::soap::validate_complete_xml(xml).ok()?;
        let request = Request::parse(xml).ok()?;
        let fault = request.envelope_operation(SOAP, "Fault").ok()?;
        let value = fault
            .child(SOAP, "Code")
            .ok()??
            .child(SOAP, "Value")
            .ok()??;
        let text = value
            .scalar_text()
            .ok()?
            .trim_matches([' ', '\t', '\r', '\n']);
        let (prefix, local) = text.split_once(':').unwrap_or(("", text));
        if value.namespace_for_prefix(prefix) != Some(SOAP) {
            return None;
        }
        match local {
            "Sender" => Some(StatusCode::BAD_REQUEST),
            "Receiver" | "VersionMismatch" | "MustUnderstand" | "DataEncodingUnknown" => {
                Some(StatusCode::INTERNAL_SERVER_ERROR)
            }
            _ => None,
        }
    };
    code().unwrap_or(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn parameters_preserve_case_and_quoted_delimiters() {
        for value in [
            "application/soap+xml; action=\"urn:Case;Sensitive\"",
            " Application/SOAP+XML ; CHARSET=\"UTF-8\"; ACTION = \"urn:Case;Sensitive\"; x=\"semi;quote\\\"\\\\\"",
        ] {
            assert_eq!(action(&headers(value)), Ok("urn:Case;Sensitive".into()));
        }
        assert_eq!(action(&headers("application/soap+xml")), Ok(String::new()));
        for good in [
            "urn:x",
            "http://example.test/a%20b?x=y",
            "a+b.c-d:x",
            "urn:Case;Sensitive?x=/a?b",
            "http://user:pass@[2001:db8::1]:8080/a",
            "http://[vF.address:part]/a",
            "scheme://host:/a",
            "urn:x%5By%5D",
        ] {
            assert!(absolute_action(good), "{good}");
        }
    }

    #[test]
    fn malformed_ambiguous_and_unsupported_headers_refuse() {
        for value in [
            "application/soap+xml;",
            "application/soap+xml; action=\"\"",
            "application/soap+xml; action=\"relative\"",
            "application/soap+xml; action=urn:x",
            "application/soap+xml; action=\"urn:x\"; Action=\"urn:x\"",
            "application/soap+xml; charset=utf-8; charset=utf-8",
            "application/soap+xml; x=one; X=two",
            "application/soap+xml; action=\"urn:x\"junk",
            "application/soap+xml; action=\"urn:x",
            "application/soap+xml; =x",
            "application/soap+xml; x=",
            "application/soap+xml; x=one;",
            "application/soap+xml; action=\"urn:x#fragment\"",
            "application/soap+xml; action=\"urn:x%Q1\"",
        ] {
            assert_eq!(
                action(&headers(value)),
                Err(StatusCode::BAD_REQUEST),
                "{value}"
            );
        }
        for value in [
            "text/xml",
            "application/json",
            "application/soap+xml; charset=utf-16",
        ] {
            assert_eq!(
                action(&headers(value)),
                Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
            );
        }
        assert_eq!(
            action(&HeaderMap::new()),
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        );
        let mut duplicate = headers("application/soap+xml");
        duplicate.append(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/soap+xml"),
        );
        assert_eq!(action(&duplicate), Err(StatusCode::BAD_REQUEST));
        let mut encoded = headers("application/soap+xml");
        encoded.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("IDENTITY"),
        );
        assert!(action(&encoded).is_ok());
        encoded.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        assert_eq!(action(&encoded), Err(StatusCode::UNSUPPORTED_MEDIA_TYPE));
        encoded.append(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        assert_eq!(action(&encoded), Err(StatusCode::BAD_REQUEST));
        for invalid in [
            "1urn:x",
            ":x",
            "urn:",
            "urn:x y",
            "urn:x%",
            "urn:x%0",
            "urn:x\\y",
            "urn:北",
            "urn:x#y",
            "urn:bad[part]/GetDeviceInformation",
            "http://[not-an-ip]/GetDeviceInformation",
            "http://example.test:abc/GetDeviceInformation",
            "http://user@@host/a",
            "http://[::1]junk/a",
            "http://[::1]:abc/a",
            "http://[v.not-a-version]/a",
            "http://[v1.]/a",
            "urn:x?bad[query]",
        ] {
            assert!(!absolute_action(invalid), "{invalid}");
        }
    }

    #[test]
    fn content_type_limit_is_inclusive() {
        let prefix = "application/soap+xml; x=";
        let value = format!("{prefix}{}", "a".repeat(8192 - prefix.len()));
        assert!(action(&headers(&value)).is_ok());
        assert_eq!(
            action(&headers(&(value + "a"))),
            Err(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn status_uses_scoped_direct_soap_fault_code_only() {
        use super::super::helpers::{resp_soap_fault, soap};
        for (code, status) in [
            ("Sender", 400),
            ("Receiver", 500),
            ("VersionMismatch", 500),
            ("MustUnderstand", 500),
            ("DataEncodingUnknown", 500),
        ] {
            assert_eq!(
                response_status(&resp_soap_fault(&format!("env:{code}"), "literal")),
                status
            );
            let body = format!(
                "<s:Fault><s:Code><s:Value xmlns:q='{SOAP}'> q:{code} </s:Value></s:Code></s:Fault>"
            );
            assert_eq!(response_status(&soap("", &body)), status);
        }
        for body in [
            "<s:Fault><s:Code><s:Value xmlns:env='urn:wrong'>env:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value>missing:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value>s:Unknown</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value><s:Value>s:Receiver</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value><s:Sender/></s:Value></s:Code></s:Fault>",
            "<response><s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault></response>",
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault xmlns:s='urn:decoy'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault><s:Fault/>",
            "<s:Fault x='<'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault 1bad='x'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><1bad/></s:Fault>",
        ] {
            assert_eq!(response_status(&soap("", body)), 200, "{body}");
        }
        assert_eq!(
            response_status(&format!(
                "<s:Fault xmlns:s='{SOAP}'><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>"
            )),
            200
        );
        assert_eq!(response_status("broken raw XML"), 200);
        assert_eq!(
            response_status(&resp_soap_fault("ter:NoProfile", "legacy")),
            200
        );
        let default =
            format!("<s:Fault><s:Code><s:Value xmlns='{SOAP}'>Sender</s:Value></s:Code></s:Fault>");
        assert_eq!(response_status(&soap("", &default)), 400);
        let fault = soap(
            "",
            "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code></s:Fault>",
        );
        assert_eq!(
            response_status(&fault.replace("</s:Envelope>", "<s:Body/></s:Envelope>")),
            200
        );
    }

    #[test]
    fn status_inspection_limits_preserve_raw_fallback() {
        use super::super::helpers::soap;
        let wrap = |payload: &str| {
            soap(
                "",
                &format!(
                    "<s:Fault><s:Code><s:Value>s:Sender</s:Value></s:Code><s:Detail>{payload}</s:Detail></s:Fault>"
                ),
            )
        };
        // These pin the scoped parser's documented inspection policy, distinct
        // from the larger production client's limits.
        let overhead = wrap("").len();
        let at_bytes = wrap(&" ".repeat(2 * 1024 * 1024 - overhead));
        assert_eq!(response_status(&at_bytes), 400);
        assert_eq!(response_status(&(at_bytes + " ")), 200);
        for (nested, expected) in [(60, 400), (61, 200)] {
            let payload = format!("{}{}", "<x>".repeat(nested), "</x>".repeat(nested));
            assert_eq!(response_status(&wrap(&payload)), expected);
        }
        // Envelope, Body, Fault, Code, Value and Detail occupy six nodes.
        for (children, expected) in [(16_384 - 6, 400), (16_384 - 5, 200)] {
            assert_eq!(response_status(&wrap(&"<x/>".repeat(children))), expected);
        }
    }
}
