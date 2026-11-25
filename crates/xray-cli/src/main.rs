mod config;
mod http;
mod init;

use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    process::{Command as Process, ExitCode, Stdio},
};

use clap::{Args, Parser, Subcommand};
use config::{Loaded, Overrides};
use serde_json::{json, Value};
use xray_api::LocalView;
use xray_collector::runtime;
use xray_core::TraceId;

const DEMO: &[u8] = include_bytes!("../../../tests/fixtures/otlp/distributed.pb");

#[derive(Parser)]
#[command(
    name = "request-xray",
    version,
    about = "Explore local OpenTelemetry traces"
)]
struct Cli {
    #[command(flatten)]
    flags: Overrides,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the local collector and query API.
    Serve,
    /// Start the collector, query API and local browser view.
    Dev(Browser),
    /// Start an isolated instance with a synthetic trace.
    Demo(Browser),
    /// Generate project config and a Node instrumentation preload.
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    /// Check the configured local collector and query API.
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// List saved traces or show a specified trace.
    Traces(Box<Traces>),
    /// Inspect persisted spans, parents and links for a trace.
    Inspect(Inspect),
    /// Validate project configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print the native CLI version.
    Version,
}

#[derive(Args)]
struct Browser {
    /// Print the browser URL without opening a window.
    #[arg(long)]
    no_open: bool,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Validate and summarize the effective local configuration.
    Validate {
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
    },
}

#[derive(Args)]
struct Traces {
    #[command(subcommand)]
    action: Option<TraceAction>,
    #[command(flatten)]
    query: TraceList,
}

#[derive(Subcommand)]
enum TraceAction {
    /// List traces ordered by most recent start time.
    List(TraceList),
    /// Show one trace and its span page.
    Show(Inspect),
}

#[derive(Default, Args)]
struct TraceList {
    #[arg(long)]
    service: Option<String>,
    #[arg(long)]
    route: Option<String>,
    #[arg(long)]
    error: bool,
    #[arg(long)]
    since_ns: Option<String>,
    #[arg(long)]
    until_ns: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: Option<u32>,
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct Inspect {
    id: TraceId,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: Option<u32>,
    #[arg(long)]
    cursor: Option<String>,
    #[arg(long)]
    json: bool,
    /// Group the current page by parent, marking parents outside the page.
    #[arg(long)]
    tree: bool,
}

struct Failure(u8, &'static str);

#[tokio::main]
async fn main() -> ExitCode {
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(code, message)) => {
            eprintln!("request-xray: {message}");
            ExitCode::from(code)
        }
    }
}

async fn execute(cli: Cli) -> Result<(), Failure> {
    if matches!(cli.command, Command::Version) {
        println!("request-xray {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if let Command::Init { dry_run } = cli.command {
        let (path, _) = config::config_path(cli.flags.config);
        return init::run(&path, dry_run)
            .map_err(|_| Failure(1, "could not write the project setup files"));
    }
    let file = match &cli.command {
        Command::Config {
            action: ConfigAction::Validate { file },
        } => file.clone(),
        _ => None,
    };
    let mut loaded = config::load(&cli.flags, file).map_err(|message| Failure(2, message))?;
    match cli.command {
        Command::Serve => start(loaded, None, false).await,
        Command::Dev(browser) => start(loaded, Some(LocalView::Live), !browser.no_open).await,
        Command::Demo(browser) => {
            let temporary = tempfile::tempdir()
                .map_err(|_| Failure(1, "could not create an isolated demo directory"))?;
            loaded.settings.database = temporary.path().join("demo.sqlite");
            loaded.settings.retention = Default::default();
            // The demo fixture needs standard request limits and deterministic visibility.
            loaded.settings.collector.limits = Default::default();
            loaded.settings.collector.admission = Default::default();
            loaded.settings.collector.redaction = Default::default();
            let result = start(loaded, Some(LocalView::Demo), !browser.no_open).await;
            drop(temporary);
            result
        }
        Command::Doctor { json } => doctor(&loaded, json).await,
        Command::Inspect(args) => inspect(&loaded, args).await,
        Command::Traces(args) => match args.action {
            Some(TraceAction::Show(args)) => inspect(&loaded, args).await,
            Some(TraceAction::List(args)) => list(&loaded, args).await,
            None => list(&loaded, args.query).await,
        },
        Command::Config { .. } => {
            println!("Configuration is valid ({})", source(&loaded));
            println!("OTLP http://{}/v1/traces", loaded.settings.collector.listen);
            println!("Query http://{}/api/v1", loaded.settings.query_listen);
            Ok(())
        }
        Command::Init { .. } | Command::Version => unreachable!(),
    }
}

async fn start(mut loaded: Loaded, view: Option<LocalView>, open: bool) -> Result<(), Failure> {
    eprintln!("Configuration: {}", source(&loaded));
    loaded.settings.view = view;
    runtime::run(loaded.settings, |endpoints| async move {
        if matches!(view, Some(LocalView::Demo)) {
            let client = http::client().map_err(io::Error::other)?;
            http::export(&client, endpoints.otlp, DEMO)
                .await
                .map_err(io::Error::other)?;
            eprintln!("Demo loaded: derived from a synthetic exporter fixture");
        }
        if let (Some(_), Some(address)) = (view, endpoints.query) {
            let url = format!("http://{address}/");
            println!("Local trace view: {url}");
            if open && io::stdout().is_terminal() {
                open_browser(&url);
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| {
        eprintln!("{error}");
        Failure(
            if error.kind() == io::ErrorKind::AddrInUse {
                3
            } else {
                1
            },
            "could not run the local collector",
        )
    })
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Process::new("open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Process::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut command = Process::new("xdg-open");
        command.arg(url);
        command
    };

    match command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut opener) => {
            std::thread::spawn(move || {
                let _ = opener.wait();
            });
        }
        Err(_) => eprintln!("Open the local trace view URL in your browser."),
    }
}

fn source(loaded: &Loaded) -> String {
    loaded.source.as_ref().map_or_else(
        || "defaults and environment".into(),
        |path| terminal(&path.display().to_string()),
    )
}

async fn doctor(loaded: &Loaded, as_json: bool) -> Result<(), Failure> {
    let client = http::client().map_err(|message| Failure(1, message))?;
    let (query, export) = tokio::join!(
        http::get(&client, loaded.settings.query_listen, "/api/v1/health", &[]),
        http::export(&client, loaded.settings.collector.listen, &[])
    );
    let query_ok = query.is_ok_and(|health| health["status"] == "ok");
    let export_ok = export.is_ok();
    if as_json {
        print_json(&json!({
            "config": source(loaded), "nativeVersion": env!("CARGO_PKG_VERSION"),
            "queryReady": query_ok, "otlpReady": export_ok,
        }))?;
    } else {
        println!("Configuration: {}", source(loaded));
        println!("Native CLI: {}", env!("CARGO_PKG_VERSION"));
        println!(
            "Query API and SQLite read readiness: {}",
            if query_ok { "ok" } else { "unavailable" }
        );
        println!(
            "OTLP protobuf probe: {}",
            if export_ok { "ok" } else { "unavailable" }
        );
        println!("Verify application preload and persistence by sending a traced request.");
    }
    if query_ok && export_ok {
        Ok(())
    } else {
        Err(Failure(
            3,
            "local collector checks failed; run request-xray dev",
        ))
    }
}

async fn list(loaded: &Loaded, args: TraceList) -> Result<(), Failure> {
    let mut filters = Vec::new();
    for (key, value) in [
        ("service", args.service),
        ("route", args.route),
        ("sinceNs", args.since_ns),
        ("untilNs", args.until_ns),
        ("cursor", args.cursor),
        ("limit", args.limit.map(|limit| limit.to_string())),
        ("error", args.error.then(|| "true".to_owned())),
    ] {
        if let Some(value) = value {
            filters.push((key.to_owned(), value));
        }
    }
    let client = http::client().map_err(|message| Failure(1, message))?;
    let page = http::get(
        &client,
        loaded.settings.query_listen,
        "/api/v1/traces",
        &filters,
    )
    .await
    .map_err(query_failure)?;
    if args.json {
        return print_json(&page);
    }
    let items = items(&page)?;
    if items.is_empty() {
        println!("No traces found.");
    }
    for item in items {
        println!(
            "{}  {} ns  {} spans  {}",
            field(item, "traceId"),
            field(item, "durationNs"),
            item["spanCount"],
            field(item, "status")
        );
    }
    next(&page);
    Ok(())
}

async fn inspect(loaded: &Loaded, args: Inspect) -> Result<(), Failure> {
    let client = http::client().map_err(|message| Failure(1, message))?;
    let address = loaded.settings.query_listen;
    let path = format!("/api/v1/traces/{}", args.id);
    let trace = http::get(&client, address, &path, &[])
        .await
        .map_err(query_failure)?;
    let mut filters = Vec::new();
    if let Some(limit) = args.limit {
        filters.push(("limit".into(), limit.to_string()));
    }
    if let Some(cursor) = args.cursor {
        filters.push(("cursor".into(), cursor));
    }
    let page = http::get(&client, address, &format!("{path}/spans"), &filters)
        .await
        .map_err(query_failure)?;
    if args.json {
        return print_json(&json!({ "trace": trace, "spans": page }));
    }
    println!(
        "Trace {} · {} ns · {} missing parents",
        args.id,
        field(&trace, "durationNs"),
        trace["completeness"]["missingParentCount"]
    );
    let spans = items(&page)?;
    if args.tree {
        let mut shown = std::collections::HashSet::new();
        for span in spans {
            let parent = span["parentSpanId"].as_str();
            if parent.is_none_or(|id| {
                !spans
                    .iter()
                    .any(|candidate| candidate["spanId"].as_str() == Some(id))
            }) {
                tree(span, spans, &mut shown, 0);
            }
        }
        for span in spans {
            tree(span, spans, &mut shown, 0);
        }
    } else {
        for span in spans {
            print_span(span, 0, spans);
        }
    }
    next(&page);
    Ok(())
}

fn tree<'a>(
    span: &'a Value,
    spans: &'a [Value],
    shown: &mut std::collections::HashSet<&'a str>,
    depth: usize,
) {
    let Some(id) = span["spanId"].as_str() else {
        return;
    };
    if !shown.insert(id) {
        return;
    }
    print_span(span, depth, spans);
    for child in spans
        .iter()
        .filter(|child| child["parentSpanId"].as_str() == Some(id))
    {
        tree(child, spans, shown, depth + 1);
    }
}

fn print_span(span: &Value, depth: usize, page: &[Value]) {
    let parent = span["parentSpanId"]
        .as_str()
        .map(|parent| {
            let suffix = if page
                .iter()
                .any(|span| span["spanId"].as_str() == Some(parent))
            {
                ""
            } else {
                " (outside page or missing)"
            };
            format!(" parent {}{suffix}", terminal(parent))
        })
        .unwrap_or_default();
    let indent = "  ".repeat(depth);
    println!(
        "{indent}{} {} · {} ns{parent}",
        field(span, "spanId"),
        field(span, "name"),
        field(span, "durationNs")
    );
    if let Some(links) = span["links"].as_array() {
        for link in links {
            println!(
                "{indent}  link {}/{}",
                field(link, "traceId"),
                field(link, "spanId")
            );
        }
    }
}

fn items(page: &Value) -> Result<&[Value], Failure> {
    page["items"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(Failure(3, "invalid local API page"))
}

fn next(page: &Value) {
    if let Some(cursor) = page["nextCursor"].as_str() {
        println!("Next cursor: {}", terminal(cursor));
    }
}

fn field(value: &Value, name: &str) -> String {
    terminal(value[name].as_str().unwrap_or("?"))
}

fn terminal(input: &str) -> String {
    input
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn print_json(value: &Value) -> Result<(), Failure> {
    serde_json::to_writer_pretty(io::stdout().lock(), value)
        .map_err(|_| Failure(1, "could not write JSON"))?;
    println!();
    Ok(())
}

fn query_failure(message: &'static str) -> Failure {
    Failure(
        if message == "invalid query filters or cursor" {
            2
        } else {
            3
        },
        message,
    )
}
