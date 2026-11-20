//! Count protobuf fields before Prost allocates repeated messages.
//!
//! Walk only the length-delimited fields declared as embedded messages in the
//! pinned trace schema. Prost still owns full decoding and type validation.

use prost::encoding::{decode_key, decode_varint, skip_field, WireType};

#[derive(Debug)]
pub(crate) enum PreflightError {
    Invalid,
    Limit,
}

pub(crate) fn check(payload: &[u8], max_fields: usize) -> Result<(), PreflightError> {
    walk(payload, Message::Export, &mut { max_fields }, 0)
}

fn walk(
    mut input: &[u8],
    message: Message,
    remaining: &mut usize,
    depth: usize,
) -> Result<(), PreflightError> {
    if depth > 64 {
        return Err(PreflightError::Limit);
    }

    while !input.is_empty() {
        *remaining = remaining.checked_sub(1).ok_or(PreflightError::Limit)?;
        let (tag, wire) = decode_key(&mut input).map_err(|_| PreflightError::Invalid)?;
        if let Some(child) = message.child(tag) {
            if wire != WireType::LengthDelimited {
                return Err(PreflightError::Invalid);
            }
            let length = decode_varint(&mut input).map_err(|_| PreflightError::Invalid)?;
            let length = usize::try_from(length).map_err(|_| PreflightError::Invalid)?;
            let (embedded, rest) = input
                .split_at_checked(length)
                .ok_or(PreflightError::Invalid)?;
            walk(embedded, child, remaining, depth + 1)?;
            input = rest;
        } else {
            skip_field(wire, tag, &mut input, Default::default())
                .map_err(|_| PreflightError::Invalid)?;
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum Message {
    Export,
    ResourceSpans,
    ScopeSpans,
    Resource,
    Scope,
    Span,
    Event,
    Link,
    Leaf,
    Attribute,
    Value,
    Array,
    AttributeList,
}

impl Message {
    fn child(self, tag: u32) -> Option<Self> {
        use Message::*;
        Some(match (self, tag) {
            (Export, 1) => ResourceSpans,
            (ResourceSpans, 1) => Resource,
            (ResourceSpans, 2) => ScopeSpans,
            (ScopeSpans, 1) => Scope,
            (ScopeSpans, 2) => Span,
            (Resource, 1)
            | (Scope, 3)
            | (Span, 9)
            | (Event, 3)
            | (Link, 4)
            | (AttributeList, 1) => Attribute,
            (Span, 11) => Event,
            (Span, 13) => Link,
            (Span, 15) | (Resource, 3) => Leaf, // Status and EntityRef
            (Attribute, 2) | (Array, 1) => Value,
            (Value, 5) => Array,
            (Value, 6) => AttributeList,
            _ => return None,
        })
    }
}
