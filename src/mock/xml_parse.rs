//! Minimal XML tag extraction for parsing SOAP request bodies.
//!
//! Handles namespace prefixes (`<tt:Name>`, `<wsse:Password>`) and
//! tags with attributes (`<wsse:Password Type="...">value</wsse:Password>`).

/// Extract the text content of the first tag with the given local name.
///
/// Matches `<ns:Name>`, `<Name>`, `<ns:Name attr="...">` — any tag whose
/// local name (after the last `:`) equals `local_name`.
pub fn extract_tag(xml: &str, local_name: &str) -> Option<String> {
    let (_, content_start, _) = find_open_tag(xml, local_name, 0)?;
    let rest = &xml[content_start..];
    let close_pos = find_close_tag(rest, local_name)?;
    Some(rest[..close_pos].trim().to_string())
}

/// Extract all values of a repeated tag.
pub fn extract_all_tags(xml: &str, local_name: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut search_from = 0;

    while search_from < xml.len() {
        let Some((_, content_start, _)) = find_open_tag(xml, local_name, search_from) else {
            break;
        };
        let rest = &xml[content_start..];
        if let Some(close_pos) = find_close_tag(rest, local_name) {
            results.push(rest[..close_pos].trim().to_string());
            search_from = content_start + close_pos;
        } else {
            search_from = content_start;
        }
    }

    results
}

/// Find the opening tag for `local_name` starting from `from`.
/// Returns `(tag_start, content_start, tag_name)` — content_start is right after `>`.
fn find_open_tag(xml: &str, local_name: &str, from: usize) -> Option<(usize, usize, String)> {
    let mut pos = from;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find('<')?;
        let abs_lt = pos + lt;
        let after_lt = &xml[abs_lt + 1..];

        // Skip closing tags, processing instructions, comments
        if after_lt.starts_with('/') || after_lt.starts_with('?') || after_lt.starts_with('!') {
            pos = abs_lt + 2;
            continue;
        }

        // Find the end of this tag
        let Some(gt) = after_lt.find('>') else {
            break;
        };
        let tag_content = &after_lt[..gt]; // e.g. "wsse:Password Type=\"...\""

        // Extract the tag name (before any space/attributes)
        let tag_name = tag_content
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");

        // Get local part (after last ':')
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        if local == local_name {
            let content_start = abs_lt + 1 + gt + 1; // position after '>'
            return Some((abs_lt, content_start, tag_name.to_string()));
        }

        pos = abs_lt + 1;
    }
    None
}

/// Extract an attribute value from the first occurrence of a tag.
///
/// e.g. for `<tt:PanTilt x="0.5" y="0.2"/>`, `extract_attr(xml, "PanTilt", "x")`
/// returns `Some("0.5")`. Handles both single- and double-quoted values.
pub fn extract_attr(xml: &str, tag_local: &str, attr: &str) -> Option<String> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find('<')?;
        let abs_lt = pos + lt;
        let after_lt = &xml[abs_lt + 1..];
        if after_lt.starts_with('/') || after_lt.starts_with('?') || after_lt.starts_with('!') {
            pos = abs_lt + 2;
            continue;
        }
        let Some(gt) = after_lt.find('>') else {
            break;
        };
        let header = &after_lt[..gt];
        let tag_name = header
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);
        if local == tag_local {
            for &quote in &['"', '\''] {
                let needle = format!("{attr}={quote}");
                if let Some(start) = header.find(&needle) {
                    let val_start = start + needle.len();
                    if let Some(end) = header[val_start..].find(quote) {
                        return Some(header[val_start..val_start + end].to_string());
                    }
                }
            }
            return None;
        }
        pos = abs_lt + 1;
    }
    None
}

/// Find the position of the closing tag `</...local_name>` relative to the input.
fn find_close_tag(xml: &str, local_name: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find("</")?;
        let abs_lt = pos + lt;
        let after = &xml[abs_lt + 2..];

        let Some(gt) = after.find('>') else { break };
        let tag_name = after[..gt].trim();
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        if local == local_name {
            return Some(abs_lt);
        }

        pos = abs_lt + 2;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_tag() {
        let xml = r#"<tt:Name>hello</tt:Name>"#;
        assert_eq!(extract_tag(xml, "Name"), Some("hello".to_string()));
    }

    #[test]
    fn extract_no_namespace() {
        let xml = r#"<Name>world</Name>"#;
        assert_eq!(extract_tag(xml, "Name"), Some("world".to_string()));
    }

    #[test]
    fn extract_nested() {
        let xml = r#"<Body><tt:Username>admin</tt:Username></Body>"#;
        assert_eq!(extract_tag(xml, "Username"), Some("admin".to_string()));
    }

    #[test]
    fn extract_tag_with_attributes() {
        let xml =
            r#"<wsse:Password Type="http://example.com#PasswordDigest">abc123==</wsse:Password>"#;
        assert_eq!(extract_tag(xml, "Password"), Some("abc123==".to_string()));
    }

    #[test]
    fn extract_nonce_with_encoding_type() {
        let xml =
            r#"<wsse:Nonce EncodingType="http://example.com#Base64Binary">bm9uY2U=</wsse:Nonce>"#;
        assert_eq!(extract_tag(xml, "Nonce"), Some("bm9uY2U=".to_string()));
    }

    #[test]
    fn extract_all() {
        let xml = r#"<a><tt:IPv4Address>8.8.8.8</tt:IPv4Address><tt:IPv4Address>1.1.1.1</tt:IPv4Address></a>"#;
        let v = extract_all_tags(xml, "IPv4Address");
        assert_eq!(v, vec!["8.8.8.8", "1.1.1.1"]);
    }

    #[test]
    fn extract_missing() {
        assert_eq!(extract_tag("<a>b</a>", "Missing"), None);
    }

    #[test]
    fn extract_from_full_soap_security_header() {
        let xml = r#"<s:Envelope>
          <s:Header>
            <wsse:Security>
              <wsse:UsernameToken>
                <wsse:Username>admin</wsse:Username>
                <wsse:Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">digest==</wsse:Password>
                <wsse:Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">bm9uY2U=</wsse:Nonce>
                <wsu:Created>2026-04-15T00:00:00Z</wsu:Created>
              </wsse:UsernameToken>
            </wsse:Security>
          </s:Header>
          <s:Body><tds:GetHostname/></s:Body>
        </s:Envelope>"#;

        assert_eq!(extract_tag(xml, "Username"), Some("admin".to_string()));
        assert_eq!(extract_tag(xml, "Password"), Some("digest==".to_string()));
        assert_eq!(extract_tag(xml, "Nonce"), Some("bm9uY2U=".to_string()));
        assert_eq!(
            extract_tag(xml, "Created"),
            Some("2026-04-15T00:00:00Z".to_string())
        );
    }
}
