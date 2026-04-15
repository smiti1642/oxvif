//! Minimal XML tag extraction for parsing SOAP request bodies.
//!
//! Handles namespace prefixes: `<tt:Name>`, `<Name>`, `<tds:Name>` all match "Name".

/// Extract the text content of the first tag with the given local name.
pub fn extract_tag(xml: &str, local_name: &str) -> Option<String> {
    // Find opening tag: anything ending with `LocalName>` or `LocalName ...>`
    let search = format!("{local_name}>");
    for (pos, _) in xml.match_indices(&search) {
        // Walk back to find '<'
        let before = &xml[..pos];
        let lt = before.rfind('<')?;
        let between = &xml[lt + 1..pos];
        // Skip closing tags
        if between.contains('/') {
            continue;
        }
        // Found opening — extract content until closing tag
        let content_start = pos + search.len();
        let rest = &xml[content_start..];
        // Find closing: </...LocalName>
        let close = format!("{local_name}>");
        for (cpos, _) in rest.match_indices(&close) {
            let cbefore = &rest[..cpos];
            if let Some(clt) = cbefore.rfind('<') {
                let cbetween = &rest[clt + 1..cpos];
                if cbetween.contains('/') {
                    return Some(rest[..clt].trim().to_string());
                }
            }
        }
    }
    None
}

/// Extract all values of a repeated tag.
/// e.g., multiple `<tt:IPv4Address>` tags.
pub fn extract_all_tags(xml: &str, local_name: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut search_from = 0;
    let search = format!("{local_name}>");

    while search_from < xml.len() {
        let rest = &xml[search_from..];
        let Some(pos) = rest.find(&search) else { break };
        let abs_pos = search_from + pos;

        // Check it's an opening tag
        let before = &xml[..abs_pos];
        let Some(lt) = before.rfind('<') else {
            search_from = abs_pos + search.len();
            continue;
        };
        let between = &xml[lt + 1..abs_pos];
        if between.contains('/') {
            search_from = abs_pos + search.len();
            continue;
        }

        let content_start = abs_pos + search.len();
        let after = &xml[content_start..];
        // Find closing tag
        if let Some(end) = find_close(after, local_name) {
            results.push(after[..end].trim().to_string());
            search_from = content_start + end;
        } else {
            search_from = content_start;
        }
    }

    results
}

fn find_close(xml: &str, local_name: &str) -> Option<usize> {
    let pattern = format!("{local_name}>");
    for (pos, _) in xml.match_indices(&pattern) {
        if pos > 0 {
            let before = &xml[..pos];
            if let Some(lt) = before.rfind('<') {
                let between = &xml[lt + 1..pos];
                if between.contains('/') {
                    return Some(lt);
                }
            }
        }
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
    fn extract_all() {
        let xml = r#"<a><tt:IPv4Address>8.8.8.8</tt:IPv4Address><tt:IPv4Address>1.1.1.1</tt:IPv4Address></a>"#;
        let v = extract_all_tags(xml, "IPv4Address");
        assert_eq!(v, vec!["8.8.8.8", "1.1.1.1"]);
    }

    #[test]
    fn extract_missing() {
        assert_eq!(extract_tag("<a>b</a>", "Missing"), None);
    }
}
