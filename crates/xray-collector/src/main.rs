use std::{env, io};
use xray_collector::runtime::{run, RuntimeSettings};

#[tokio::main]
async fn main() -> io::Result<()> {
    let validate_only = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--validate-only" => true,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: xray-collector [--validate-only]",
            ))
        }
    };
    let mut settings = RuntimeSettings {
        validate_only,
        ..Default::default()
    };
    settings.apply_env()?;
    run(settings, |_| async { Ok(()) }).await
}
