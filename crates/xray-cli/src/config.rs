use std::{
    env,
    fs::File,
    io::{self, Read},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Args;
use serde::Deserialize;
use xray_collector::runtime::RuntimeSettings;

pub const DEFAULT_CONFIG: &str = "request-xray.config.json";
const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Args)]
pub struct Overrides {
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(long, global = true)]
    pub db_path: Option<PathBuf>,
    #[arg(long, global = true, conflicts_with = "collector_listen")]
    pub collector_port: Option<u16>,
    #[arg(
        long,
        global = true,
        conflicts_with = "query_listen",
        visible_alias = "dashboard-port"
    )]
    pub query_port: Option<u16>,
    #[arg(long, global = true)]
    pub collector_listen: Option<SocketAddr>,
    #[arg(long, global = true)]
    pub query_listen: Option<SocketAddr>,
    #[arg(long, hide = true)]
    pub managed_stdin: bool,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct Project {
    #[serde(rename = "$schema")]
    _schema: Option<String>,
    version: Option<u32>,
    collector_listen: Option<SocketAddr>,
    query_listen: Option<SocketAddr>,
    database_path: Option<PathBuf>,
    retention_age_seconds: Option<u64>,
    retention_max_bytes: Option<u64>,
    max_compressed_bytes: Option<usize>,
    max_decoded_bytes: Option<usize>,
    max_in_flight: Option<usize>,
    body_timeout_ms: Option<u64>,
    sink_timeout_ms: Option<u64>,
    allow_attributes: Option<Vec<String>>,
    deny_attributes: Option<Vec<String>>,
}

impl Project {
    fn apply(self, settings: &mut RuntimeSettings, path: &Path) -> Result<(), &'static str> {
        if self.version.is_some_and(|version| version != 1) {
            return Err("unsupported configuration version");
        }
        if let Some(value) = self.collector_listen {
            settings.collector.listen = value;
        }
        if let Some(value) = self.query_listen {
            settings.query_listen = value;
        }
        if let Some(value) = self.database_path {
            if value.as_os_str().is_empty() {
                return Err("databasePath must not be empty");
            }
            settings.database = path.parent().unwrap_or(Path::new("")).join(value);
        }
        if let Some(value) = self.retention_age_seconds {
            settings.retention.max_age = Duration::from_secs(value);
        }
        if let Some(value) = self.retention_max_bytes {
            settings.retention.max_bytes = value;
        }
        if let Some(value) = self.max_compressed_bytes {
            settings.collector.limits.compressed_bytes = value;
        }
        if let Some(value) = self.max_decoded_bytes {
            settings.collector.limits.decoded_bytes = value;
        }
        if let Some(value) = self.max_in_flight {
            settings.collector.admission.max_in_flight = value;
        }
        if let Some(value) = self.body_timeout_ms {
            settings.collector.admission.body_timeout = Duration::from_millis(value);
        }
        if let Some(value) = self.sink_timeout_ms {
            settings.collector.admission.sink_timeout = Duration::from_millis(value);
        }
        settings.collector.redaction.allow = self.allow_attributes;
        settings.collector.redaction.deny = self.deny_attributes.unwrap_or_default();
        Ok(())
    }
}

pub struct Loaded {
    pub settings: RuntimeSettings,
    pub source: Option<PathBuf>,
}

pub fn config_path(explicit: Option<PathBuf>) -> (PathBuf, bool) {
    match explicit.or_else(|| env::var_os("REQUEST_XRAY_CONFIG").map(PathBuf::from)) {
        Some(path) => (path, true),
        None => (DEFAULT_CONFIG.into(), false),
    }
}

pub fn load(flags: &Overrides, file: Option<PathBuf>) -> Result<Loaded, &'static str> {
    let (path, required) = config_path(file.or_else(|| flags.config.clone()));
    let mut settings = RuntimeSettings::default();
    let source = match File::open(&path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "could not read project config")?;
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err("project config exceeds 64 KiB");
            }
            let file: Project = serde_json::from_slice(&bytes)
                .map_err(|_| "invalid project config JSON or field type; see config.schema.json")?;
            file.apply(&mut settings, &path)?;
            Some(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && !required => None,
        Err(_) => return Err("could not open project config"),
    };
    settings
        .apply_env()
        .map_err(|_| "invalid Request X-Ray environment setting")?;
    apply_port_env(&mut settings)?;
    if let Some(value) = flags.db_path.clone() {
        settings.database = value;
    }
    if let Some(value) = flags.collector_listen {
        settings.collector.listen = value;
    }
    if let Some(value) = flags.query_listen {
        settings.query_listen = value;
    }
    if let Some(value) = flags.collector_port {
        settings.collector.listen.set_port(value);
    }
    if let Some(value) = flags.query_port {
        settings.query_listen.set_port(value);
    }
    settings.managed_stdin = flags.managed_stdin;
    validate(&settings)?;
    Ok(Loaded { settings, source })
}

fn apply_port_env(settings: &mut RuntimeSettings) -> Result<(), &'static str> {
    for (name, address) in [
        (
            "REQUEST_XRAY_COLLECTOR_PORT",
            &mut settings.collector.listen,
        ),
        ("REQUEST_XRAY_DASHBOARD_PORT", &mut settings.query_listen),
    ] {
        if let Some(port) = env::var_os(name) {
            let port = port
                .to_str()
                .and_then(|value| value.parse().ok())
                .ok_or("invalid Request X-Ray port environment setting")?;
            address.set_port(port);
        }
    }
    Ok(())
}

fn validate(settings: &RuntimeSettings) -> Result<(), &'static str> {
    let collector = &settings.collector;
    let query = settings.query_listen;
    if !collector.listen.ip().is_loopback() || !query.ip().is_loopback() {
        return Err("collector and query addresses must be loopback");
    }
    if collector.listen == query && query.port() != 0 {
        return Err("collector and query listeners must use different addresses");
    }
    if settings.database.as_os_str().is_empty() {
        return Err("database path must not be empty");
    }
    let body = collector.admission.body_timeout;
    let sink = collector.admission.sink_timeout;
    if ![body, sink]
        .into_iter()
        .all(|duration| (1..=120_000).contains(&duration.as_millis()))
    {
        return Err("timeouts must be between 1 and 120000 ms");
    }
    if ![
        collector.limits.compressed_bytes,
        collector.limits.decoded_bytes,
    ]
    .into_iter()
    .all(|bytes| (1..=64 * 1024 * 1024).contains(&bytes))
        || !(1..=64).contains(&collector.admission.max_in_flight)
    {
        return Err("invalid body size or concurrent request limit");
    }
    for list in collector
        .redaction
        .allow
        .iter()
        .chain(std::iter::once(&collector.redaction.deny))
    {
        if list.len() > 128
            || list
                .iter()
                .any(|key| key.is_empty() || key.len() > 256 || key.chars().any(char::is_control))
        {
            return Err("invalid attribute key list");
        }
    }
    Ok(())
}
