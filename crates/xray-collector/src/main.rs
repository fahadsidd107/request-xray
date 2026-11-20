use std::{env, error::Error, io, time::Duration};

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
    let mode = if validate_only {
        "validation only"
    } else {
        "storage unavailable"
    };
    eprintln!(
        "OTLP/HTTP listener at http://{}/v1/traces ({mode})",
        listener.local_addr()?
    );
    let ingestion = Ingestion::new(config);
    ingestion
        .serve(
            listener,
            move |_batch| async move {
                if validate_only {
                    Ok(SinkOutcome::ValidationOnly)
                } else {
                    Err(StorageUnavailable)
                }
            },
            shutdown,
        )
        .await?;
    eprintln!(
        "OTLP {} diagnostics: {:?}",
        Diagnostics::SOURCE_ENDPOINT,
        ingestion.diagnostics()
    );
    Ok(())
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
