use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{any_value::Value, KeyValue},
};

/// Limits are checked against the wire model before conversion or redaction.
#[derive(Clone)]
pub struct Limits {
    pub compressed_bytes: usize,
    pub decoded_bytes: usize,
    pub protobuf_fields: usize,
    pub spans: usize,
    pub events_per_span: usize,
    pub links_per_span: usize,
    pub attributes_per_list: usize,
    pub array_items: usize,
    pub value_bytes: usize,
    pub attribute_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            compressed_bytes: 4 * 1024 * 1024,
            decoded_bytes: 16 * 1024 * 1024,
            protobuf_fields: 100_000,
            spans: 10_000,
            events_per_span: 128,
            links_per_span: 128,
            attributes_per_list: 128,
            array_items: 128,
            value_bytes: 16 * 1024,
            attribute_depth: 8,
        }
    }
}

impl Limits {
    pub(crate) fn permits(&self, request: &ExportTraceServiceRequest) -> bool {
        let mut remaining_spans = self.spans;
        for resource_group in &request.resource_spans {
            if let Some(resource) = &resource_group.resource {
                if !self.attributes_fit(&resource.attributes, 0) {
                    return false;
                }
            }
            for scope_group in &resource_group.scope_spans {
                if scope_group.spans.len() > remaining_spans {
                    return false;
                }
                remaining_spans -= scope_group.spans.len();

                if let Some(scope) = &scope_group.scope {
                    if !self.attributes_fit(&scope.attributes, 0) {
                        return false;
                    }
                }
                for span in &scope_group.spans {
                    if span.events.len() > self.events_per_span
                        || span.links.len() > self.links_per_span
                        || !self.attributes_fit(&span.attributes, 0)
                        || !span
                            .events
                            .iter()
                            .all(|event| self.attributes_fit(&event.attributes, 0))
                        || !span
                            .links
                            .iter()
                            .all(|link| self.attributes_fit(&link.attributes, 0))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn attributes_fit(&self, attributes: &[KeyValue], depth: usize) -> bool {
        attributes.len() <= self.attributes_per_list
            && attributes.iter().all(|attribute| {
                attribute.key.len() <= self.value_bytes
                    && self.value_fits(
                        attribute
                            .value
                            .as_ref()
                            .and_then(|value| value.value.as_ref()),
                        depth,
                    )
            })
    }

    fn value_fits(&self, value: Option<&Value>, depth: usize) -> bool {
        if depth > self.attribute_depth {
            return false;
        }
        match value {
            Some(Value::StringValue(value)) => value.len() <= self.value_bytes,
            Some(Value::BytesValue(value)) => value.len() <= self.value_bytes,
            Some(Value::ArrayValue(array)) => {
                depth < self.attribute_depth
                    && array.values.len() <= self.array_items
                    && array
                        .values
                        .iter()
                        .all(|value| self.value_fits(value.value.as_ref(), depth + 1))
            }
            Some(Value::KvlistValue(list)) => {
                depth < self.attribute_depth && self.attributes_fit(&list.values, depth + 1)
            }
            _ => true,
        }
    }
}
