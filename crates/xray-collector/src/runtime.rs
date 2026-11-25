//! Shared standalone collector lifecycle used by the native and npm commands.

use crate::{Config, Diagnostics, Ingestion, SinkOutcome, StorageUnavailable};
use std::{
    env,
    future::Future,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use xray_storage::{Repository, Retention};

pub struct RuntimeSettings {
    pub collector: Config,
    pub query_listen: SocketAddr,
    pub database: PathBuf,
    pub retention: Retention,
    pub validate_only: bool,
    pub managed_stdin: bool,
    pub view: Option<xray_api::LocalView>,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            collector: Config::default(),
            query_listen: xray_api::DEFAULT_LISTEN,
            database: ".request-xray/traces.sqlite".into(),
            retention: Retention::default(),
            validate_only: false,
            managed_stdin: false,
            view: None,
        }
    }
}

impl RuntimeSettings {
    pub fn apply_env(&mut self) -> io::Result<()> {
        let config = &mut self.collector;
        if let Some(address) = env_value("REQUEST_XRAY_OTLP_LISTEN")? {
            config.listen = address.parse().map_err(invalid_env)?;
        }
        for (name, limit) in [
            (
                "REQUEST_XRAY_MAX_COMPRESSED_BYTES",
                &mut config.limits.compressed_bytes,
            ),
            (
                "REQUEST_XRAY_MAX_DECODED_BYTES",
                &mut config.limits.decoded_bytes,
            ),
        ] {
            if let Some(value) = env_value(name)? {
                *limit = value.parse().map_err(invalid_env)?;
            }
        }
        if let Some(value) = env_value("REQUEST_XRAY_MAX_IN_FLIGHT")? {
            config.admission.max_in_flight = value.parse().map_err(invalid_env)?;
        }
        for (name, timeout) in [
            (
                "REQUEST_XRAY_BODY_TIMEOUT_MS",
                &mut config.admission.body_timeout,
            ),
            (
                "REQUEST_XRAY_SINK_TIMEOUT_MS",
                &mut config.admission.sink_timeout,
            ),
        ] {
            if let Some(value) = env_value(name)? {
                *timeout = Duration::from_millis(value.parse().map_err(invalid_env)?);
            }
        }
        if let Some(value) = env_value("REQUEST_XRAY_ALLOW_ATTRIBUTES")? {
            config.redaction.allow = Some(keys(value));
        }
        if let Some(value) = env_value("REQUEST_XRAY_DENY_ATTRIBUTES")? {
            config.redaction.deny = keys(value);
        }

        if let Some(address) = env_value("REQUEST_XRAY_QUERY_LISTEN")? {
            self.query_listen = address.parse().map_err(invalid_env)?;
        }
        if let Some(path) = env::var_os("REQUEST_XRAY_DB_PATH") {
            self.database = path.into();
        }
        if let Some(seconds) = env_value("REQUEST_XRAY_RETENTION_AGE_SECONDS")? {
            self.retention.max_age = Duration::from_secs(seconds.parse().map_err(invalid_env)?);
        }
        if let Some(bytes) = env_value("REQUEST_XRAY_RETENTION_MAX_BYTES")? {
            self.retention.max_bytes = bytes.parse().map_err(invalid_env)?;
        }
        Ok(())
    }
}

fn invalid_env(_: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid Request X-Ray environment setting",
    )
}

#[derive(Clone, Copy)]
pub struct Endpoints {
    pub otlp: SocketAddr,
    pub query: Option<SocketAddr>,
}

fn listener_error(name: &str, address: SocketAddr, error: io::Error) -> io::Error {
    let detail = if error.kind() == io::ErrorKind::AddrInUse {
        "address already in use"
    } else {
        "could not bind loopback listener"
    };
    io::Error::new(error.kind(), format!("{name} {address}: {detail}"))
}

pub async fn run<F>(settings: RuntimeSettings, ready: impl FnOnce(Endpoints) -> F) -> io::Result<()>
where
    F: Future<Output = io::Result<()>>,
{
    let RuntimeSettings {
        collector: config,
        query_listen,
        database,
        retention,
        validate_only,
        managed_stdin,
        view,
    } = settings;
    let listener = config
        .bind()
        .await
        .map_err(|error| listener_error("OTLP", config.listen, error))?;
    let signal = shutdown_signal()?;
    let managed = managed_stdin.then(watch_parent).transpose()?;
    let query_listener = if validate_only {
        None
    } else {
        Some(
            xray_api::bind(query_listen)
                .await
                .map_err(|error| listener_error("query", query_listen, error))?,
        )
    };
    let storage = if validate_only {
        None
    } else {
        Some(open_storage(database, retention).await?)
    };
    let query = query_listener.zip(
        storage
            .as_ref()
            .map(|(repository, _, _)| repository.clone()),
    );
    let mode = if validate_only {
        "validation only"
    } else {
        "SQLite storage"
    };
    eprintln!(
        "OTLP/HTTP listener at http://{}/v1/traces ({mode})",
        listener.local_addr()?
    );

    if let Some((listener, _)) = &query {
        eprintln!("Query API at http://{}/api/v1", listener.local_addr()?);
    }

    let endpoints = Endpoints {
        otlp: listener.local_addr()?,
        query: query
            .as_ref()
            .map(|(listener, _)| listener.local_addr())
            .transpose()?,
    };

    let retention_stats = storage.as_ref().map(|(_, _, initial)| {
        Arc::new(Mutex::new(xray_api::RetentionStats {
            deleted_traces: initial.deleted_traces,
            live_bytes_at_last_pass: initial.live_bytes,
            failures: 0,
        }))
    });
    let maintenance = storage.as_ref().zip(retention_stats.as_ref()).map(
        |((repository, retention, _), stats)| {
            let stats = stats.clone();
            let repository = repository.clone();
            let retention = *retention;
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                interval.tick().await;
                loop {
                    interval.tick().await;
                    match repository.enforce_retention(retention).await {
                        Ok(report) => {
                            let mut stats = stats.lock().expect("retention diagnostics lock");
                            stats.deleted_traces += report.deleted_traces;
                            stats.live_bytes_at_last_pass = report.live_bytes;
                            if report.deleted_traces != 0 || report.live_bytes > retention.max_bytes
                            {
                                eprintln!(
                                    "SQLite retention removed {} traces, {} live bytes",
                                    report.deleted_traces, report.live_bytes
                                );
                            }
                        }
                        Err(_) => {
                            stats.lock().expect("retention diagnostics lock").failures += 1;
                            eprintln!("SQLite retention failed");
                        }
                    }
                }
            })
        },
    );
    let ingestion = Ingestion::new(config);
    let repository = storage
        .as_ref()
        .map(|(repository, _, _)| repository.clone());
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let signal_task = tokio::spawn(async move {
        if let Some(parent) = managed {
            tokio::select! {
                _ = signal => {},
                _ = parent => {},
            }
        } else {
            signal.await;
        }
        let _ = stop.send(true);
    });
    let ingest = ingestion.serve(
        listener,
        move |batch| {
            let repository = repository.clone();
            async move {
                let Some(repository) = repository else {
                    return Ok(SinkOutcome::ValidationOnly);
                };
                let report = repository
                    .write_batch(batch)
                    .await
                    .map_err(|_| StorageUnavailable)?;
                Ok(SinkOutcome::Committed {
                    duplicate_spans: report.duplicate_spans,
                    conflicting_spans: report.conflicting_spans,
                })
            }
        },
        wait_for_shutdown(stopped.clone()),
    );
    let diagnostic_source = retention_stats.map(|retention| {
        let ingestion = ingestion.clone();
        let source: xray_api::DiagnosticsSource = Arc::new(move || xray_api::RuntimeStats {
            ingestion: ingestion_stats(ingestion.diagnostics()),
            retention: retention
                .lock()
                .expect("retention diagnostics lock")
                .clone(),
        });
        source
    });
    let query = async {
        if let Some((listener, repository)) = query {
            xray_api::serve_with_view(
                listener,
                repository,
                diagnostic_source,
                view,
                wait_for_shutdown(stopped),
            )
            .await?;
        }
        Ok(())
    };
    let result = tokio::try_join!(ingest, query, ready(endpoints));
    signal_task.abort();
    let _ = signal_task.await;
    if let Some(task) = maintenance {
        task.abort();
        let _ = task.await;
    }
    if let Some((repository, _, _)) = storage {
        repository.close().await;
    }
    result?;
    eprintln!(
        "OTLP {} diagnostics: {:?}",
        Diagnostics::SOURCE_ENDPOINT,
        ingestion.diagnostics()
    );
    Ok(())
}

fn ingestion_stats(stats: Diagnostics) -> xray_api::IngestionStats {
    xray_api::IngestionStats {
        source_endpoint: Diagnostics::SOURCE_ENDPOINT,
        accepted_spans: stats.accepted_spans,
        rejected_spans: stats.rejected_spans,
        duplicate_spans: stats.duplicate_spans,
        conflicting_spans: stats.conflicting_spans,
        redacted_spans: stats.redacted_spans,
        redacted_fields: stats.redacted_fields,
        validation_only_spans: stats.validation_only_spans,
        size_errors: stats.size_errors,
        invalid_payloads: stats.invalid_payloads,
        admission_rejections: stats.admission_rejections,
        shutdown_rejections: stats.shutdown_rejections,
        body_timeouts: stats.body_timeouts,
        write_failures: stats.write_failures,
        write_timeouts: stats.write_timeouts,
    }
}

async fn open_storage(
    path: PathBuf,
    retention: Retention,
) -> io::Result<(Repository, Retention, xray_storage::RetentionReport)> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let repository = Repository::open(path).await.map_err(io::Error::other)?;
    let initial = repository
        .enforce_retention(retention)
        .await
        .map_err(io::Error::other)?;
    Ok((repository, retention, initial))
}

async fn wait_for_shutdown(mut stopped: tokio::sync::watch::Receiver<bool>) {
    let _ = stopped.wait_for(|stop| *stop).await;
}

fn watch_parent() -> io::Result<tokio::sync::oneshot::Receiver<()>> {
    use std::io::Read;

    let (end, closed) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("xray-launcher-shutdown".into())
        .spawn(move || {
            // The launcher owns this pipe and closes it on shutdown or process exit.
            let _ = io::stdin().lock().read(&mut [0_u8; 1]);
            let _ = end.send(());
        })?;
    Ok(closed)
}

fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        Ok(async move {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = interrupt.recv() => {},
            }
        })
    }
    #[cfg(not(unix))]
    Ok(async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("could not listen for Ctrl-C: {error}");
        }
    })
}

fn env_value(name: &str) -> io::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(invalid_env(error)),
    }
}

fn keys(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .collect()
}
