use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest.join("../../schemas/vendor/opentelemetry-proto");
    let license = proto_root.join("LICENSE");
    let inputs = [
        "opentelemetry/proto/common/v1/common.proto",
        "opentelemetry/proto/resource/v1/resource.proto",
        "opentelemetry/proto/trace/v1/trace.proto",
        "opentelemetry/proto/collector/trace/v1/trace_service.proto",
    ];
    if !license.is_file() {
        return Err(format!(
            "Missing upstream OTLP sources. Run: bash scripts/otlp-sync.sh ({})",
            license.display()
        )
        .into());
    }
    let files: Vec<_> = inputs.iter().map(|p| proto_root.join(p)).collect();
    for path in &files {
        if !path.is_file() {
            return Err(format!(
                "Missing protobuf file: {}. Run: bash scripts/otlp-sync.sh",
                path.display()
            )
            .into());
        }
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // This is a build-script process-scoped setting, not an application runtime change.
    env::set_var("PROTOC", protoc);
    prost_build::Config::new().compile_protos(&files, &[proto_root])?;
    Ok(())
}
