// SPDX-License-Identifier: AGPL-3.0-or-later

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let workspace = crate_dir.join("../..");
    let asset_root = workspace.join("editor/dist");
    println!("cargo:rerun-if-changed={}", asset_root.display());
    println!("cargo:rerun-if-env-changed=WORKFLOWD_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    if !asset_root.join("index.html").is_file() {
        panic!("editor assets are missing; run `npm ci && npm run build` in editor before Cargo");
    }

    let mut files = Vec::new();
    collect_files(&asset_root, &asset_root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut generated = String::from("pub const EMBEDDED_ASSETS: &[EmbeddedAsset] = &[\n");
    for (relative, absolute) in &files {
        let bytes = fs::read(absolute).expect("read editor asset");
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let route = format!("/{}", relative.replace('\\', "/"));
        let immutable = relative.starts_with("assets/");
        generated.push_str(&format!(
            "    EmbeddedAsset {{ path: {:?}, content_type: {:?}, etag: {:?}, immutable: {}, bytes: include_bytes!({:?}) }},\n",
            route,
            content_type(relative),
            format!("\"sha256-{digest}\""),
            immutable,
            absolute.canonicalize().expect("canonical asset path")
        ));
    }
    generated.push_str("];\n");

    let manifest_digest = format!(
        "{:x}",
        Sha256::digest(
            fs::read(asset_root.join("asset-manifest.json")).expect("read editor asset manifest")
        )
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    fs::write(output.join("embedded_assets.rs"), generated).expect("write embedded asset table");

    println!("cargo:rustc-env=WORKFLOWD_EDITOR_MANIFEST_SHA256={manifest_digest}");
    println!(
        "cargo:rustc-env=WORKFLOWD_BUILD_COMMIT={}",
        env::var("WORKFLOWD_BUILD_COMMIT").unwrap_or_else(|_| "development".into())
    );
    println!(
        "cargo:rustc-env=WORKFLOWD_SOURCE_DATE_EPOCH={}",
        env::var("SOURCE_DATE_EPOCH").unwrap_or_else(|_| "0".into())
    );
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).expect("read editor asset directory") {
        let entry = entry.expect("read editor asset entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output);
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("asset beneath root")
                .to_string_lossy()
                .into_owned();
            output.push((relative, path));
        }
    }
}

fn content_type(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|value| value.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
