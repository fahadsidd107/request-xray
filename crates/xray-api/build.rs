use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=XRAY_REQUIRE_DASHBOARD");
    println!("cargo:rustc-check-cfg=cfg(xray_dashboard)");
    let dist = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../packages/dashboard/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("dashboard.rs");
    let Ok(index) = fs::read(dist.join("index.html")) else {
        assert!(
            env::var("XRAY_REQUIRE_DASHBOARD").as_deref() != Ok("1"),
            "build the dashboard before compiling the release CLI"
        );
        return;
    };
    let mut assets: Vec<_> = fs::read_dir(dist.join("assets"))
        .expect("dashboard assets")
        .collect::<Result<_, _>>()
        .expect("read dashboard assets");
    assets.sort_by_key(|asset| asset.file_name());
    assert!(!assets.is_empty(), "dashboard asset bundle is empty");

    let index = String::from_utf8(index).expect("dashboard HTML must be UTF-8");
    assert!(
        index.contains("data-mode=\"live\""),
        "dashboard index must include its mode marker"
    );
    let mut source = format!("const INDEX: &str = {index:?};\n");
    source.push_str("const ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for asset in assets {
        let path = asset.path().canonicalize().expect("dashboard asset path");
        assert!(
            asset.file_type().expect("asset type").is_file(),
            "dashboard assets must be regular files"
        );
        let name = asset
            .file_name()
            .into_string()
            .expect("asset name must be UTF-8");
        let mime = match path.extension().and_then(|extension| extension.to_str()) {
            Some("js") => "text/javascript; charset=utf-8",
            Some("css") => "text/css; charset=utf-8",
            Some("woff2") => "font/woff2",
            _ => panic!("unsupported dashboard asset: {name}"),
        };
        source.push_str(&format!(
            "({name:?}, {mime:?}, include_bytes!({:?})),\n",
            path.to_str().unwrap()
        ));
    }
    source.push_str("];\n");
    fs::write(out, source).expect("generate embedded dashboard");
    println!("cargo:rustc-cfg=xray_dashboard");
}
