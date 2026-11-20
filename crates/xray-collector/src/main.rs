use std::{env, error::Error, io, path::PathBuf, time::Duration};

use xray_storage::{Repository, Retention};

use xray_collector::{Config, Diagnostics, Ingestion, SinkOutcome, StorageUnavailable};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let validate_only = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--validate-only" => true,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: xray-collector [--validate-only]",
            )
            .into())
        }
    };

    let mut config = Config::default();
    if let Some(address) = env_value("REQUEST_XRAY_OTLP_LISTEN")? {
        config.listen = address.parse()?;
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
            *limit = value.parse()?;
        }
    }
    if let Some(value) = env_value("REQUEST_XRAY_MAX_IN_FLIGHT")? {
        config.admission.max_in_flight = value.parse()?;
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
            *timeout = Duration::from_millis(value.parse()?);
        }
    }
    config.redaction.allow = env_value("REQUEST_XRAY_ALLOW_ATTRIBUTES")?.map(keys);
    config.redaction.deny = env_value("REQUEST_XRAY_DENY_ATTRIBUTES")?.map_or_else(Vec::new, keys);

    let listener = config.bind().await?;
    let shutdown = shutdown_signal()?;
    let storage = if validate_only {
        None
    } else {
        Some(open_storage().await?)
    };
    let mode = if validate_only {
        "validation only"
    } else {
        "SQLite storage"
    };
    eprintln!(
        "OTLP/HTTP listener at http://{}/v1/traces ({mode})",
        listener.local_addr()?
    );

    let maintenance = storage.as_ref().map(|(repository, retention)| {
        let repository = repository.clone();
        let retention = *retention;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await;
            loop {
                interval.tick().await;
                match repository.enforce_retention(retention).await {
                    Ok(report)
                        if report.deleted_traces != 0
                            || report.live_bytes > retention.max_bytes =>
                    {
                        eprintln!(
                            "SQLite retention removed {} traces, {} live bytes",
                            report.deleted_traces, report.live_bytes
                        );
                    }
                    Ok(_) => {}
                    Err(_) => eprintln!("SQLite retention failed"),
                }
            }
        })
    });
    let ingestion = Ingestion::new(config);
    let repository = storage.as_ref().map(|(repository, _)| repository.clone());
    let result = ingestion
        .serve(
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
            shutdown,
        )
        .await;
    if let Some(task) = maintenance {
        task.abort();
        let _ = task.await;
    }
    if let Some((repository, _)) = storage {
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

async fn open_storage() -> Result<(Repository, Retention), Box<dyn Error>> {
    let path = env::var_os("REQUEST_XRAY_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".request-xray/traces.sqlite"));
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut retention = Retention::default();
    if let Some(seconds) = env_value("REQUEST_XRAY_RETENTION_AGE_SECONDS")? {
        retention.max_age = Duration::from_secs(seconds.parse()?);
    }
    if let Some(bytes) = env_value("REQUEST_XRAY_RETENTION_MAX_BYTES")? {
        retention.max_bytes = bytes.parse()?;
    }
    let repository = Repository::open(path).await.map_err(io::Error::other)?;
    repository
        .enforce_retention(retention)
        .await
        .map_err(io::Error::other)?;
    Ok((repository, retention))
}

fn shutdown_signal() -> io::Result<impl std::future::Future<Output = ()>> {
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

fn env_value(name: &str) -> Result<Option<String>, env::VarError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error),
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
