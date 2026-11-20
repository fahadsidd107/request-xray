use xray_core::{AttributeValue, Attributes, ResourceSpans};

/// Additional case-insensitive exact attribute keys. An allowlist restricts
/// capture when set. Defaults and custom denials always take precedence.
#[derive(Clone, Default)]
pub struct Redaction {
    pub allow: Option<Vec<String>>,
    pub deny: Vec<String>,
}

impl Redaction {
    pub(crate) fn sanitize(&self, resources: &mut [ResourceSpans]) -> (u64, u64) {
        let mut fields = 0;
        let mut spans = 0;
        for group in resources {
            if let Some(resource) = &mut group.resource {
                self.attributes(&mut resource.attributes, &mut fields);
                let entity_count = resource.entity_refs.len();
                resource.entity_refs.retain(|entity| {
                    entity
                        .id_keys
                        .iter()
                        .chain(&entity.description_keys)
                        .all(|key| {
                            resource
                                .attributes
                                .iter()
                                .any(|attribute| attribute.key == *key)
                        })
                });
                fields += (entity_count - resource.entity_refs.len()) as u64;
            }
            for scope_group in &mut group.scope_spans {
                if let Some(scope) = &mut scope_group.scope {
                    self.attributes(&mut scope.attributes, &mut fields);
                }
                for span in &mut scope_group.spans {
                    let before = fields;
                    self.attributes(&mut span.attributes, &mut fields);
                    fields += u64::from(sanitize_url(&mut span.name, false));
                    if let Some(status) = &mut span.status {
                        fields += u64::from(!status.message.is_empty());
                        status.message.clear();
                    }
                    for event in &mut span.events {
                        self.attributes(&mut event.attributes, &mut fields);
                        fields += u64::from(sanitize_url(&mut event.name, false));
                    }
                    for link in &mut span.links {
                        self.attributes(&mut link.attributes, &mut fields);
                    }
                    spans += u64::from(fields != before);
                }
            }
        }
        (spans, fields)
    }

    fn attributes(&self, attributes: &mut Attributes, fields: &mut u64) {
        attributes.retain_mut(|attribute| {
            let key = attribute.key.to_ascii_lowercase();
            if denied_by_default(&key)
                || self
                    .deny
                    .iter()
                    .any(|denied| key.eq_ignore_ascii_case(denied))
                || self.allow.as_ref().is_some_and(|allow| {
                    !allow
                        .iter()
                        .any(|allowed| key.eq_ignore_ascii_case(allowed))
                })
            {
                *fields += 1;
                return false;
            }
            let url_key = key.starts_with("url.")
                || key == "url"
                || matches!(key.as_str(), "http.url" | "http.target" | "http.route");
            if let Some(value) = &mut attribute.value {
                self.value(value, url_key, fields);
            }
            true
        });
    }

    fn value(&self, value: &mut AttributeValue, url_key: bool, fields: &mut u64) {
        match value {
            AttributeValue::String(value) => *fields += u64::from(sanitize_url(value, url_key)),
            AttributeValue::Array(values) => {
                for value in values {
                    self.value(value, url_key, fields);
                }
            }
            AttributeValue::KeyValueList(attributes) => self.attributes(attributes, fields),
            _ => {}
        }
    }
}

fn denied_by_default(key: &str) -> bool {
    // Match across common spellings such as api_key, api-key and apiKey.
    let normalized: String = key.chars().filter(char::is_ascii_alphanumeric).collect();
    matches!(
        normalized.as_str(),
        "auth" | "body" | "dbstatement" | "dbquerytext"
    ) || [
        "authorization",
        "cookie",
        "password",
        "passwd",
        "apikey",
        "token",
        "secret",
        "credential",
        "cardnumber",
        "paymentcard",
        "creditcard",
        "privatekey",
        "baggage",
        "requestbody",
        "responsebody",
        "requestheader",
        "responseheader",
        "sqlparameter",
        "dbqueryparameter",
        "urlquery",
        "urlfragment",
        "exceptionmessage",
        "exceptionstacktrace",
    ]
    .iter()
    .any(|part| normalized.contains(part))
}

fn sanitize_url(value: &mut String, known_url: bool) -> bool {
    let original_len = value.len();
    if known_url {
        if let Some(index) = value.find(['?', '#']) {
            value.truncate(index);
        }
    }
    if !value.contains('/') {
        return value.len() != original_len;
    }

    // Span names can contain an HTTP verb followed by a request target.
    let mut sanitized = String::with_capacity(value.len());
    for chunk in value.split_inclusive(char::is_whitespace) {
        let part = chunk.trim_end_matches(char::is_whitespace);
        let authority = part
            .find("://")
            .map(|scheme| scheme + 3)
            .or_else(|| part.starts_with("//").then_some(2));
        let url_like = authority.is_some() || part.starts_with('/');
        let end = if url_like {
            part.find(['?', '#']).unwrap_or(part.len())
        } else {
            part.len()
        };
        let part_end = part.len();
        let target = &part[..end];
        if let Some(authority) = authority.filter(|authority| *authority <= end) {
            let host_and_path = &target[authority..];
            let host_end = host_and_path.find('/').unwrap_or(host_and_path.len());
            if let Some(at) = host_and_path[..host_end].rfind('@') {
                sanitized.push_str(&target[..authority]);
                sanitized.push_str(&host_and_path[at + 1..]);
            } else {
                sanitized.push_str(target);
            }
        } else {
            sanitized.push_str(target);
        }
        sanitized.push_str(&chunk[part_end..]);
    }
    *value = sanitized;
    value.len() != original_len
}
