//! Synthetic input for the dashboard's 1k and 10k span benchmarks.

use std::{env, fs, io, path::PathBuf};

use prost::Message;
use xray_otlp::opentelemetry::proto::{
    collector::trace::v1::ExportTraceServiceRequest,
    common::v1::{any_value::Value, AnyValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};

fn main() -> io::Result<()> {
    let output = PathBuf::from(env::args_os().nth(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "provide a fixture output directory",
        )
    })?);
    fs::create_dir_all(&output)?;
    for (count, trace_byte) in [(1_000u64, 0x43u8), (10_000, 0x44)] {
        let root = 1u64.to_be_bytes();
        let epoch = 1_793_500_000_000_000_001u64;
        let spans = (1..=count)
            .map(|index| {
                let started = epoch + index * 100;
                Span {
                    trace_id: vec![trace_byte; 16],
                    span_id: index.to_be_bytes().to_vec(),
                    parent_span_id: if index == 1 {
                        Vec::new()
                    } else {
                        root.to_vec()
                    },
                    name: if index == 1 {
                        "fixture root".into()
                    } else {
                        format!("fixture span {index}")
                    },
                    start_time_unix_nano: started,
                    end_time_unix_nano: if index == 1 {
                        epoch + 2 * count * 100
                    } else {
                        started + 150
                    },
                    ..Span::default()
                }
            })
            .collect();
        let fixture = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans,
                    ..ScopeSpans::default()
                }],
                ..ResourceSpans::default()
            }],
        };
        fs::write(output.join(format!("{count}.pb")), fixture.encode_to_vec())?;
    }
    // Enough equally timed traces to test the dashboard's next and previous page.
    let listing = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: (1..=26)
                    .map(|id| Span {
                        trace_id: vec![id; 16],
                        span_id: vec![id; 8],
                        name: "pagination fixture".into(),
                        start_time_unix_nano: 1_793_600_000_000_000_000,
                        end_time_unix_nano: 1_793_600_000_000_000_010,
                        ..Span::default()
                    })
                    .collect(),
                ..ScopeSpans::default()
            }],
            ..ResourceSpans::default()
        }],
    };
    fs::write(output.join("listing.pb"), listing.encode_to_vec())?;
    let base = 1_793_700_000_000_000_000;
    let late_group = |service: &str, id: u8, parent: Vec<u8>, start, end, dropped| {
        ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                resource: Some(Resource {
                    attributes: vec![KeyValue {
                        key: "service.name".into(),
                        value: Some(AnyValue {
                            value: Some(Value::StringValue(service.into())),
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                scope_spans: vec![ScopeSpans {
                    spans: vec![Span {
                        trace_id: vec![0x45; 16],
                        span_id: vec![id; 8],
                        parent_span_id: parent,
                        name: service.into(),
                        start_time_unix_nano: start,
                        end_time_unix_nano: end,
                        dropped_attributes_count: dropped,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
        .encode_to_vec()
    };
    fs::write(
        output.join("missing-parent.pb"),
        late_group("fixture child", 2, vec![1; 8], base, base + 80, 2),
    )?;
    fs::write(
        output.join("late-parent.pb"),
        late_group("fixture parent", 1, vec![], base + 20, base + 90, 0),
    )?;
    Ok(())
}
