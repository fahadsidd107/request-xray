use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};

use serde_json::{json, Value};

pub fn run(config: &Path, dry_run: bool) -> io::Result<()> {
    let project = read_manifest();
    let esm = project.get("type").and_then(Value::as_str) == Some("module");
    let service = project
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control))
        .unwrap_or("request-xray-app");
    let service = serde_json::to_string(service)?;
    let preload = if esm {
        "request-xray.instrumentation.mjs"
    } else {
        "request-xray.instrumentation.cjs"
    };
    let source = if esm {
        format!("process.env.OTEL_SERVICE_NAME ??= {service};\nawait import('@request-xray/node/register');\n")
    } else {
        format!("process.env.OTEL_SERVICE_NAME ??= {service};\nrequire('@request-xray/node/register');\n")
    };
    let mut data = serde_json::to_string_pretty(&json!({
        "$schema": "./node_modules/request-xray/config.schema.json",
        "version": 1,
    }))?;
    data.push('\n');
    create(config, &data, dry_run)?;
    create(Path::new(preload), &source, dry_run)?;
    let flag = if esm { "--import" } else { "--require" };
    println!("Add @request-xray/node to your application and start it with node {flag} ./{preload} <entrypoint>.");
    println!("Use REQUEST_XRAY_COLLECTOR_ENDPOINT if your collector uses a custom address.");
    Ok(())
}

fn read_manifest() -> Value {
    let Ok(file) = File::open("package.json") else {
        return Value::Null;
    };
    let mut bytes = Vec::new();
    if file.take(64 * 1024 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > 64 * 1024 {
        return Value::Null;
    }
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn create(path: &Path, content: &str, dry_run: bool) -> io::Result<()> {
    if dry_run {
        if path.exists() {
            println!("Keep {}", path.display());
        } else {
            println!("Create {}\n{content}", path.display());
        }
        return Ok(());
    }
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(content.as_bytes())?;
            println!("Created {}", path.display());
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            println!("Kept existing {}", path.display());
            Ok(())
        }
        Err(error) => Err(error),
    }
}
