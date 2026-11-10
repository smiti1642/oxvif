//! Deliberately limited namespace-aware response guard before local-name parsing.
use super::{
    ComparisonError as Error, ComparisonMask, ComparisonOptions, ComparisonOutcome as Outcome,
    ComparisonProjection as Projection, ComparisonReason as Reason, ComparisonRow, Fixture,
    ParseStatus,
};
use quick_xml::{
    NsReader, XmlVersion,
    events::{Event, attributes::Attribute},
    name::{QName, ResolveResult},
};
use serde_json::{Map, Value, json};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const TDS: &str = "http://www.onvif.org/ver10/device/wsdl";
const MEDIA2: &str = "http://www.onvif.org/ver20/media/wsdl";
const TT: &str = "http://www.onvif.org/ver10/schema";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const PROFILE_FIELDS: &[&str] = &[
    "token",
    "name",
    "video_source_config_token",
    "video_source_token",
    "video_encoder_token",
    "audio_source_token",
    "audio_encoder_token",
    "ptz_config_token",
];

pub(super) fn projection(action: &str) -> Option<Projection> {
    match action {
        "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation" => {
            Some(Projection::DeviceInformation)
        }
        "http://www.onvif.org/ver10/device/wsdl/GetHostname" => Some(Projection::Hostname),
        "http://www.onvif.org/ver10/media/wsdl/GetProfiles"
        | "http://www.onvif.org/ver20/media/wsdl/GetProfiles" => Some(Projection::Profiles),
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations"
        | "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations" => {
            Some(Projection::EncoderCore)
        }
        _ => None,
    }
}

pub(super) fn validate_masks(masks: &[ComparisonMask]) -> Result<(), Error> {
    if masks.len() > 32 {
        return Err(Error::Limit {
            resource: "field masks",
            limit: 32,
        });
    }
    for mask in masks {
        let allowed = match projection(&mask.action) {
            Some(Projection::DeviceInformation) => matches!(
                mask.field.as_str(),
                "/manufacturer"
                    | "/model"
                    | "/firmware_version"
                    | "/serial_number"
                    | "/hardware_id"
            ),
            Some(Projection::Hostname) => matches!(mask.field.as_str(), "/name" | "/from_dhcp"),
            Some(Projection::Profiles) => mask.field == "/profiles/*/name",
            Some(Projection::EncoderCore) => matches!(
                mask.field.as_str(),
                "/configurations/*/name"
                    | "/configurations/*/encoding"
                    | "/configurations/*/resolution"
                    | "/configurations/*/bitrate_limit"
                    | "/configurations/*/multicast_address"
            ),
            None => false,
        };
        if !allowed {
            return Err(Error::InvalidMask);
        }
    }
    Ok(())
}

#[derive(Default)]
struct Node {
    id: usize,
    ns: String,
    name: String,
    attrs: BTreeMap<(String, String), String>,
    children: Vec<Node>,
    text: String,
}
enum Issue {
    Invalid,
    QName,
    Limit(Error),
}
type Checked<T> = Result<T, Issue>;

fn namespace(value: ResolveResult<'_>) -> Checked<String> {
    let result = match value {
        ResolveResult::Unbound => return Ok(String::new()),
        ResolveResult::Unknown(_) => return Err(Issue::Invalid),
        ResolveResult::Bound(ns) => {
            if ns.as_ref().len() > 1024 {
                return Err(Issue::Limit(Error::Limit {
                    resource: "namespace bytes",
                    limit: 1024,
                }));
            }
            Attribute {
                key: QName("xmlns"),
                value: Cow::Borrowed(ns.as_ref()),
            }
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| Issue::Invalid)?
            .into_owned()
        }
    };
    Ok(result)
}

fn parse(xml: &str) -> Checked<Node> {
    match crate::soap::XmlNode::parse_complete(xml) {
        Ok(_) => (),
        Err(crate::soap::SoapError::ResourceLimit { limit, .. }) => {
            return Err(Issue::Limit(Error::Limit {
                resource: "production XML",
                limit,
            }));
        }
        Err(_) => return Err(Issue::Invalid),
    }
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut nodes = 0;
    let mut attributes = 0;
    let mut declarations = 0;
    loop {
        match reader.read_event().map_err(|_| Issue::Invalid)? {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(&event, Event::Empty(_));
                let start = match event {
                    Event::Start(start) | Event::Empty(start) => start,
                    _ => return Err(Issue::Invalid),
                };
                nodes += 1;
                if nodes > 16384 {
                    return Err(Issue::Limit(Error::Limit {
                        resource: "projection XML nodes",
                        limit: 16384,
                    }));
                }
                let (ns, name) = reader.resolver().resolve_element(start.name());
                if name.as_ref().len() > 256 {
                    return Err(Issue::Limit(Error::Limit {
                        resource: "XML name bytes",
                        limit: 256,
                    }));
                }
                let mut node = Node {
                    id: nodes,
                    ns: namespace(ns)?,
                    name: name.as_ref().to_owned(),
                    ..Node::default()
                };
                for attr in start.attributes().with_checks(false) {
                    let attr = attr.map_err(|_| Issue::Invalid)?;
                    attributes += 1;
                    if attributes > 16384 {
                        return Err(Issue::Limit(Error::Limit {
                            resource: "projection attributes",
                            limit: 16384,
                        }));
                    }
                    if attr.key.as_ref() == "xmlns" || attr.key.as_ref().starts_with("xmlns:") {
                        declarations += 1;
                        if declarations > 256 {
                            return Err(Issue::Limit(Error::Limit {
                                resource: "namespace declarations",
                                limit: 256,
                            }));
                        }
                        continue;
                    }
                    let (ns, name) = reader.resolver().resolve_attribute(attr.key);
                    if name.as_ref().len() > 256 {
                        return Err(Issue::Limit(Error::Limit {
                            resource: "XML name bytes",
                            limit: 256,
                        }));
                    }
                    let ns = namespace(ns)?;
                    if ns == XSI && name.as_ref() == "type" {
                        return Err(Issue::QName);
                    }
                    let value = attr
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| Issue::Invalid)?
                        .into_owned();
                    if node
                        .attrs
                        .insert((ns, name.as_ref().to_owned()), value)
                        .is_some()
                    {
                        return Err(Issue::Invalid);
                    }
                }
                if empty {
                    place(&mut stack, &mut root, node)?;
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().ok_or(Issue::Invalid)?;
                place(&mut stack, &mut root, node)?;
            }
            Event::Text(text) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text.xml10_content());
                }
            }
            Event::CData(text) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                let value = match reference.as_ref() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => reference
                        .resolve_char_ref()
                        .map_err(|_| Issue::Invalid)?
                        .ok_or(Issue::Invalid)?,
                };
                if let Some(node) = stack.last_mut() {
                    node.text.push(value);
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    root.ok_or(Issue::Invalid)
}
fn place(stack: &mut [Node], root: &mut Option<Node>, node: Node) -> Checked<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(Issue::Invalid);
    }
    Ok(())
}

#[derive(Default)]
struct Examined {
    nodes: BTreeSet<usize>,
    attrs: BTreeSet<(usize, String, String)>,
}
impl Examined {
    fn mark(&mut self, node: &Node) {
        self.nodes.insert(node.id);
    }
    fn children<'a>(
        &mut self,
        node: &'a Node,
        ns: &str,
        name: &str,
        repeated: bool,
    ) -> Checked<Vec<&'a Node>> {
        let found = node
            .children
            .iter()
            .filter(|child| child.name == name)
            .collect::<Vec<_>>();
        if found.iter().any(|child| child.ns != ns) || (!repeated && found.len() > 1) {
            return Err(Issue::Invalid);
        }
        for child in &found {
            self.mark(child);
        }
        Ok(found)
    }
    fn child<'a>(
        &mut self,
        node: &'a Node,
        ns: &str,
        name: &str,
        required: bool,
    ) -> Checked<Option<&'a Node>> {
        let child = self.children(node, ns, name, false)?.into_iter().next();
        if required && child.is_none() {
            return Err(Issue::Invalid);
        }
        Ok(child)
    }
    fn scalar<'a>(
        &mut self,
        node: &'a Node,
        ns: &str,
        name: &str,
        required: bool,
    ) -> Checked<Option<&'a Node>> {
        let child = self.child(node, ns, name, required)?;
        if child.is_some_and(|child| !child.children.is_empty()) {
            return Err(Issue::Invalid);
        }
        Ok(child)
    }
    fn token(&mut self, node: &Node) -> Checked<()> {
        if node
            .attrs
            .keys()
            .any(|(ns, name)| name == "token" && !ns.is_empty())
        {
            return Err(Issue::Invalid);
        }
        if node
            .attrs
            .get(&(String::new(), "token".into()))
            .is_none_or(String::is_empty)
        {
            return Err(Issue::Invalid);
        }
        self.attrs.insert((node.id, String::new(), "token".into()));
        Ok(())
    }
}

struct Guarded {
    seen: Examined,
    fault: bool,
    destinations: BTreeMap<String, Value>,
}

fn selected_mixed(node: &Node, seen: &Examined) -> bool {
    seen.nodes.contains(&node.id)
        && ((!node.children.is_empty() && !node.text.trim().is_empty())
            || node
                .children
                .iter()
                .any(|child| selected_mixed(child, seen)))
}

fn response_guard(root: &Node, action: &str, kind: Projection) -> Checked<Guarded> {
    if root.ns != SOAP || root.name != "Envelope" {
        return Err(Issue::Invalid);
    }
    let mut seen = Examined::default();
    seen.mark(root);
    let body = seen
        .child(root, SOAP, "Body", true)?
        .ok_or(Issue::Invalid)?;
    if body.children.len() != 1 || !body.text.trim().is_empty() || !root.text.trim().is_empty() {
        return Err(Issue::Invalid);
    }
    let response = &body.children[0];
    if response.name == "Fault" {
        if response.ns != SOAP {
            return Err(Issue::Invalid);
        }
        return Ok(Guarded {
            seen,
            fault: true,
            destinations: BTreeMap::new(),
        });
    }
    let (service, operation) = action.rsplit_once('/').ok_or(Issue::Invalid)?;
    if response.ns != service || response.name != format!("{operation}Response") {
        return Err(Issue::Invalid);
    }
    seen.mark(response);
    let mut destinations = BTreeMap::new();
    match kind {
        Projection::DeviceInformation => {
            for field in [
                "Manufacturer",
                "Model",
                "FirmwareVersion",
                "SerialNumber",
                "HardwareId",
            ] {
                seen.scalar(response, TDS, field, true)?;
            }
        }
        Projection::Hostname => {
            let info = seen
                .child(response, TDS, "HostnameInformation", true)?
                .ok_or(Issue::Invalid)?;
            let dhcp = seen
                .scalar(info, TT, "FromDHCP", true)?
                .ok_or(Issue::Invalid)?;
            if !matches!(dhcp.text.trim(), "true" | "false" | "1" | "0") {
                return Err(Issue::Invalid);
            }
            seen.scalar(info, TT, "Name", false)?;
        }
        Projection::Profiles => {
            for profile in seen.children(response, service, "Profiles", true)? {
                seen.token(profile)?;
                seen.scalar(
                    profile,
                    if service == MEDIA2 { MEDIA2 } else { TT },
                    "Name",
                    true,
                )?;
                let (parent, binding_ns, fields): (&Node, &str, &[&str]) = if service == MEDIA2 {
                    let Some(parent) = seen.child(profile, MEDIA2, "Configurations", false)? else {
                        continue;
                    };
                    (
                        parent,
                        MEDIA2,
                        &[
                            "VideoSource",
                            "VideoEncoder",
                            "AudioSource",
                            "AudioEncoder",
                            "PTZ",
                        ],
                    )
                } else {
                    (
                        profile,
                        TT,
                        &[
                            "VideoSourceConfiguration",
                            "VideoEncoderConfiguration",
                            "AudioSourceConfiguration",
                            "AudioEncoderConfiguration",
                            "PTZConfiguration",
                        ],
                    )
                };
                for (index, field) in fields.iter().enumerate() {
                    if let Some(binding) = seen.child(parent, binding_ns, field, false)? {
                        seen.token(binding)?;
                        if index == 0 {
                            seen.scalar(binding, TT, "SourceToken", true)?;
                        }
                    }
                }
            }
        }
        Projection::EncoderCore => {
            for config in seen.children(response, service, "Configurations", true)? {
                seen.token(config)?;
                seen.scalar(config, TT, "Name", true)?;
                seen.scalar(config, TT, "Encoding", true)?;
                let resolution = seen
                    .child(config, TT, "Resolution", true)?
                    .ok_or(Issue::Invalid)?;
                for field in ["Width", "Height"] {
                    let scalar = seen
                        .scalar(resolution, TT, field, true)?
                        .ok_or(Issue::Invalid)?;
                    if scalar.text.trim().parse::<u32>().is_err() {
                        return Err(Issue::Invalid);
                    }
                }
                if let Some(rate) = seen.child(config, TT, "RateControl", false)? {
                    let scalar = seen
                        .scalar(rate, TT, "BitrateLimit", true)?
                        .ok_or(Issue::Invalid)?;
                    if scalar.text.trim().parse::<u32>().is_err() {
                        return Err(Issue::Invalid);
                    }
                }
                let destination =
                    if let Some(multicast) = seen.child(config, TT, "Multicast", false)? {
                        let address = seen
                            .child(multicast, TT, "Address", true)?
                            .ok_or(Issue::Invalid)?;
                        let kind = seen
                            .scalar(address, TT, "Type", true)?
                            .ok_or(Issue::Invalid)?
                            .text
                            .trim();
                        let field = match kind {
                            "IPv4" => "IPv4Address",
                            "IPv6" => "IPv6Address",
                            _ => return Err(Issue::Invalid),
                        };
                        let other = if field == "IPv4Address" {
                            "IPv6Address"
                        } else {
                            "IPv4Address"
                        };
                        if seen.scalar(address, TT, other, false)?.is_some() {
                            return Err(Issue::Invalid);
                        }
                        let value = seen
                            .scalar(address, TT, field, true)?
                            .ok_or(Issue::Invalid)?
                            .text
                            .trim();
                        if !matches!(
                            (kind, value.parse::<std::net::IpAddr>()),
                            ("IPv4", Ok(std::net::IpAddr::V4(_)))
                                | ("IPv6", Ok(std::net::IpAddr::V6(_)))
                        ) {
                            return Err(Issue::Invalid);
                        }
                        json!({"kind": kind, "value": value})
                    } else {
                        Value::Null
                    };
                let token = config
                    .attrs
                    .get(&(String::new(), "token".into()))
                    .ok_or(Issue::Invalid)?;
                destinations.insert(token.clone(), destination);
            }
        }
    }
    if selected_mixed(root, &seen) {
        return Err(Issue::Invalid);
    }
    Ok(Guarded {
        seen,
        fault: false,
        destinations,
    })
}

fn reserve_path(
    paths: &mut super::compare::PathBudget,
    options: &ComparisonOptions,
    path: &str,
) -> Result<(), Error> {
    let count = paths.count.saturating_add(1);
    if count > options.limits.field_paths {
        return Err(Error::Limit {
            resource: "field paths",
            limit: options.limits.field_paths,
        });
    }
    let bytes = paths.bytes.saturating_add(
        serde_json::to_vec(path)
            .map_err(|_| Error::InvalidLimit)?
            .len(),
    );
    if bytes > options.limits.report_bytes {
        return Err(Error::Limit {
            resource: "report bytes",
            limit: options.limits.report_bytes,
        });
    }
    paths.count = count;
    paths.bytes = bytes;
    Ok(())
}
fn unexamined(
    node: &Node,
    seen: &Examined,
    parent: &str,
    paths: &mut super::compare::PathBudget,
    options: &ComparisonOptions,
    output: &mut BTreeSet<String>,
) -> Result<(), Error> {
    let length = parent
        .len()
        .saturating_add(node.ns.len())
        .saturating_add(node.name.len())
        .saturating_add(4);
    // Internal ancestor paths are not retained unless a field is unexamined.
    // Bound their construction separately from the serialized report budget.
    if length > 64 * (1024 + 256 + 4) {
        return Err(Error::Limit {
            resource: "projection path bytes",
            limit: 64 * (1024 + 256 + 4),
        });
    }
    let path = format!("{parent}/{{{}}}{}", node.ns, node.name);
    if !seen.nodes.contains(&node.id) {
        if !output.contains(&path) {
            reserve_path(paths, options, &path)?;
            output.insert(path);
        }
        return Ok(());
    }
    for (ns, name) in node.attrs.keys() {
        if !seen.attrs.contains(&(node.id, ns.clone(), name.clone())) {
            if path
                .len()
                .saturating_add(ns.len())
                .saturating_add(name.len())
                .saturating_add(4)
                > 64 * (1024 + 256 + 4)
            {
                return Err(Error::Limit {
                    resource: "projection path bytes",
                    limit: 64 * (1024 + 256 + 4),
                });
            }
            let field = format!("{path}/@{{{ns}}}{name}");
            if !output.contains(&field) {
                reserve_path(paths, options, &field)?;
                output.insert(field);
            }
        }
    }
    for child in &node.children {
        unexamined(child, seen, &path, paths, options, output)?;
    }
    Ok(())
}

fn select_fields(value: &Value, fields: &[&str]) -> Checked<Value> {
    let mut result = Map::new();
    for field in fields {
        result.insert(
            (*field).into(),
            value.get(field).ok_or(Issue::Invalid)?.clone(),
        );
    }
    Ok(Value::Object(result))
}
fn typed_projection(
    mut value: Value,
    kind: Projection,
    destinations: &BTreeMap<String, Value>,
) -> Result<Value, Reason> {
    if kind == Projection::DeviceInformation {
        return select_fields(
            &value,
            &[
                "manufacturer",
                "model",
                "firmware_version",
                "serial_number",
                "hardware_id",
            ],
        )
        .map_err(|_| Reason::ResponseProjection);
    }
    if kind == Projection::Hostname {
        return select_fields(&value, &["name", "from_dhcp"])
            .map_err(|_| Reason::ResponseProjection);
    }
    let values = value.as_array_mut().ok_or(Reason::ResponseProjection)?;
    let mut tokens = BTreeSet::new();
    for value in values.iter() {
        let token = value
            .get("token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or(Reason::ResponseProjection)?;
        if !tokens.insert(token.to_owned()) {
            return Err(Reason::DuplicateTokens);
        }
    }
    values.sort_by(|a, b| a["token"].as_str().cmp(&b["token"].as_str()));
    let selected = values
        .iter()
        .map(|value| {
            if kind == Projection::Profiles {
                select_fields(value, PROFILE_FIELDS).map_err(|_| Reason::ResponseProjection)
            } else {
                let mut result = select_fields(value, &["token", "name", "encoding", "resolution"])
                    .map_err(|_| Reason::ResponseProjection)?;
                let bitrate = match value.get("rate_control") {
                    Some(Value::Null) => Value::Null,
                    Some(rate) => rate
                        .get("bitrate_limit")
                        .ok_or(Reason::ResponseProjection)?
                        .clone(),
                    None => return Err(Reason::ResponseProjection),
                };
                result
                    .as_object_mut()
                    .ok_or(Reason::ResponseProjection)?
                    .insert("bitrate_limit".into(), bitrate);
                let token = value
                    .get("token")
                    .and_then(Value::as_str)
                    .ok_or(Reason::ResponseProjection)?;
                let destination = destinations
                    .get(token)
                    .ok_or(Reason::ResponseProjection)?
                    .clone();
                result
                    .as_object_mut()
                    .ok_or(Reason::ResponseProjection)?
                    .insert("multicast_address".into(), destination);
                Ok(result)
            }
        })
        .collect::<Result<Vec<_>, Reason>>()?;
    Ok(if kind == Projection::Profiles {
        json!({"profiles": selected})
    } else {
        json!({"configurations": selected})
    })
}

fn apply_mask(value: &mut Value, field: &str) {
    if let Some((list, key)) = field
        .strip_prefix('/')
        .and_then(|field| field.split_once("/*/"))
    {
        if let Some(values) = value.get_mut(list).and_then(Value::as_array_mut) {
            for value in values {
                if let Some(object) = value.as_object_mut() {
                    object.remove(key);
                }
            }
        }
    } else if let Some(object) = value.as_object_mut() {
        object.remove(field.trim_start_matches('/'));
    }
}
fn differences(
    left: Option<&Value>,
    right: Option<&Value>,
    path: &str,
    options: &ComparisonOptions,
    paths: &mut super::compare::PathBudget,
    output: &mut Vec<String>,
) -> Result<(), Error> {
    if left == right {
        return Ok(());
    }
    match (left, right) {
        (Some(Value::Object(left)), Some(Value::Object(right))) => {
            for field in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
                differences(
                    left.get(field),
                    right.get(field),
                    &format!("{path}/{field}"),
                    options,
                    paths,
                    output,
                )?;
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right))) => {
            for index in 0..left.len().max(right.len()) {
                differences(
                    left.get(index),
                    right.get(index),
                    &format!("{path}/{index}"),
                    options,
                    paths,
                    output,
                )?;
            }
        }
        _ => {
            reserve_path(paths, options, path)?;
            output.push(path.to_owned());
        }
    }
    Ok(())
}

pub(super) async fn compare_pair(
    left: &Fixture,
    right: &Fixture,
    options: &ComparisonOptions,
    row: &mut ComparisonRow,
    paths: &mut super::compare::PathBudget,
) -> Result<(), Error> {
    let Some(kind) = row.projection else {
        return Ok(());
    };
    let mut values = Vec::new();
    for (is_left, fixture) in [(true, left), (false, right)] {
        let guarded = parse(&fixture.response_raw).and_then(|root| {
            response_guard(&root, &fixture.action, kind).map(|guarded| (root, guarded))
        });
        let (root, guarded) = match guarded {
            Ok(guarded) => guarded,
            Err(Issue::Limit(error)) => return Err(error),
            Err(issue) => {
                row.outcome = if matches!(issue, Issue::QName) {
                    Outcome::Unsupported
                } else {
                    Outcome::Unparseable
                };
                row.reason = if matches!(issue, Issue::QName) {
                    Reason::UnresolvedQName
                } else {
                    Reason::ResponseProjection
                };
                return Ok(());
            }
        };
        let (status, _, value) =
            super::parse::parse_check(&fixture.action, &fixture.response_raw).await;
        if guarded.fault && status == ParseStatus::Faulted {
            row.outcome = Outcome::DeviceFault;
            row.reason = Reason::DeviceFault;
            return Ok(());
        }
        if status != ParseStatus::Parsed || guarded.fault {
            row.outcome = Outcome::Unparseable;
            row.reason = Reason::ResponseProjection;
            return Ok(());
        }
        let Some(value) = value else {
            row.outcome = Outcome::Unparseable;
            row.reason = Reason::ResponseProjection;
            return Ok(());
        };
        let value = match typed_projection(value, kind, &guarded.destinations) {
            Ok(value) => value,
            Err(reason) => {
                row.outcome = if reason == Reason::DuplicateTokens {
                    Outcome::Ambiguous
                } else {
                    Outcome::Unparseable
                };
                row.reason = reason;
                return Ok(());
            }
        };
        let output = if is_left {
            &mut row.unexamined_left
        } else {
            &mut row.unexamined_right
        };
        let mut fields = BTreeSet::new();
        unexamined(&root, &guarded.seen, "", paths, options, &mut fields)?;
        *output = fields.into_iter().collect();
        values.push(value);
    }
    row.masks = options
        .masks
        .iter()
        .filter(|mask| mask.action == row.action)
        .map(|mask| mask.field.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    for field in &row.masks {
        for value in &mut values {
            apply_mask(value, field);
        }
    }
    differences(
        values.first(),
        values.get(1),
        "",
        options,
        paths,
        &mut row.changed_fields,
    )?;
    row.outcome = if row.changed_fields.is_empty() {
        Outcome::Equal
    } else {
        Outcome::Different
    };
    row.reason = if row.changed_fields.is_empty() {
        Reason::SelectedValuesEqual
    } else {
        Reason::SelectedValuesDiffer
    };
    Ok(())
}
