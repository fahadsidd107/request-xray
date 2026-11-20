use std::collections::HashSet;

use xray_core::{AttributeValue, ResourceSpans, StatusCode};

use crate::{now_ns, Repository, StorageError};

#[derive(Debug, Default, Eq, PartialEq)]
pub struct WriteReport {
    pub inserted_spans: usize,
    pub duplicate_spans: usize,
    pub conflicting_spans: usize,
}

impl Repository {
    /// Insert a sanitized batch atomically. The first committed span wins an identity conflict.
    pub async fn write_batch(
        &self,
        batch: Vec<ResourceSpans>,
    ) -> Result<WriteReport, StorageError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut report = WriteReport::default();
        let mut touched = HashSet::new();
        let now = now_ns();
        for group in batch {
            if group.scope_spans.iter().all(|scope| scope.spans.is_empty()) {
                continue;
            }
            let service = group.resource.as_ref().and_then(|resource| {
                resource
                    .attributes
                    .iter()
                    .find_map(|attr| match &attr.value {
                        Some(AttributeValue::String(name)) if attr.key == "service.name" => {
                            Some(name.as_str())
                        }
                        _ => None,
                    })
            });
            let resource = serde_json::to_string(&group.resource)?;
            sqlx::query("INSERT INTO resources(schema_url,payload,service_name) VALUES(?,?,?) ON CONFLICT(schema_url,payload) DO NOTHING")
                .bind(&group.schema_url).bind(&resource).bind(service).execute(&mut *transaction).await?;
            let resource_id: i64 =
                sqlx::query_scalar("SELECT id FROM resources WHERE schema_url = ? AND payload = ?")
                    .bind(&group.schema_url)
                    .bind(&resource)
                    .fetch_one(&mut *transaction)
                    .await?;
            let mut used_resource = false;
            for scope in group.scope_spans {
                let scope_payload = serde_json::to_string(&scope.scope)?;
                for span in scope.spans {
                    let trace = span.trace_id.as_bytes().as_slice();
                    let span_id = span.span_id.as_bytes().as_slice();
                    let payload = serde_json::to_string(&span)?;
                    let existing: Option<(i64, String, String, String)> = sqlx::query_as(
                        "SELECT resource_id, scope_schema_url, scope_payload, payload FROM spans WHERE trace_id = ? AND span_id = ?"
                    ).bind(trace).bind(span_id).fetch_optional(&mut *transaction).await?;
                    if let Some((existing_resource, schema, existing_scope, existing_span)) =
                        existing
                    {
                        if existing_resource == resource_id
                            && schema == scope.schema_url
                            && existing_scope == scope_payload
                            && existing_span == payload
                        {
                            report.duplicate_spans += 1;
                        } else {
                            report.conflicting_spans += 1;
                        }
                        continue;
                    }

                    let start = span.interval.start().as_i64();
                    let end = span.interval.end().as_i64();
                    let has_error = span
                        .status
                        .as_ref()
                        .is_some_and(|status| status.code == StatusCode::ERROR);
                    sqlx::query("INSERT INTO traces(trace_id,start_ns,end_ns,first_seen_ns,last_seen_ns) VALUES(?,?,?,?,?) ON CONFLICT(trace_id) DO NOTHING")
                        .bind(trace).bind(start).bind(end).bind(now).bind(now).execute(&mut *transaction).await?;
                    sqlx::query("INSERT INTO spans(trace_id,span_id,parent_span_id,resource_id,start_ns,end_ns,has_error,scope_schema_url,scope_payload,payload) VALUES(?,?,?,?,?,?,?,?,?,?)")
                        .bind(trace).bind(span_id).bind(span.parent_span_id.as_ref().map(|parent| parent.as_bytes().as_slice()))
                        .bind(resource_id).bind(start).bind(end).bind(has_error).bind(&scope.schema_url).bind(&scope_payload).bind(payload)
                        .execute(&mut *transaction).await?;
                    touched.insert(span.trace_id);
                    report.inserted_spans += 1;
                    used_resource = true;
                }
            }
            if !used_resource {
                sqlx::query("DELETE FROM resources WHERE id = ? AND NOT EXISTS (SELECT 1 FROM spans WHERE resource_id = ?)")
                    .bind(resource_id).bind(resource_id).execute(&mut *transaction).await?;
            }
        }

        for trace in touched {
            let id = trace.as_bytes().as_slice();
            sqlx::query(
                "UPDATE traces SET last_seen_ns = MAX(last_seen_ns, ?),
                 (start_ns, end_ns, span_count, has_error, missing_parent_count) =
                 (SELECT MIN(s.start_ns), MAX(s.end_ns), COUNT(*), MAX(s.has_error),
                  SUM(CASE WHEN s.parent_span_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM spans p WHERE p.trace_id = s.trace_id AND p.span_id = s.parent_span_id
                  ) THEN 1 ELSE 0 END) FROM spans s WHERE s.trace_id = traces.trace_id)
                 WHERE trace_id = ?"
            ).bind(now).bind(id).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(report)
    }
}
