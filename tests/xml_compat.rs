//! Synthetic compatibility fixtures captured before the quick-xml 0.42 migration.
use oxvif::soap::XmlNode;

#[test]
fn mixed_unicode_entities_and_cdata_preserve_content() {
    let node = XmlNode::parse(
        "<X>  攝影機 &amp; &#x1F4F7; &unknown;<![CDATA[ <raw>&text\r\n]]> tail\r\nend  </X>",
    )
    .unwrap();
    assert_eq!(
        node.text(),
        "攝影機 & 📷 &unknown; <raw>&text\r\n tail\nend"
    );
}

#[test]
fn attributes_normalize_whitespace_without_losing_char_refs() {
    let node = XmlNode::parse(
        "<n:攝影機 xmlns='urn:default' xmlns:n='urn:synthetic' n:名稱=\"前門\tA\r\nB&#xA;&#x9;&amp;\"/>",
    )
    .unwrap();
    assert_eq!(node.local_name, "攝影機");
    assert_eq!(node.attrs.len(), 1);
    assert_eq!(node.attr("名稱"), Some("前門 A B\n\t&"));
}

#[test]
fn malformed_and_partial_inputs_keep_existing_policy() {
    for xml in ["", "<A></B>", "</A>", "<A"] {
        assert!(XmlNode::parse(xml).is_err(), "input: {xml}");
    }
    // These are existing permissive behaviors, not a claim of XML validation.
    assert_eq!(
        XmlNode::parse("<A>unfinished").unwrap().text(),
        "unfinished"
    );
    assert_eq!(XmlNode::parse("<A/><B/>").unwrap().local_name, "A");
    assert_eq!(
        XmlNode::parse("<A key='&unknown;'/>").unwrap().attr("key"),
        Some("")
    );
    assert_eq!(
        XmlNode::parse("<A>&#x110000;</A>").unwrap().text(),
        "&#x110000;"
    );
}
