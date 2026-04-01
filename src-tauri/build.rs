use std::{fs, path::Path};

fn main() {
    let marker_path = Path::new("windows").join("portable-runtime-marker.txt");
    println!("cargo:rerun-if-changed={}", marker_path.display());

    let marker = fs::read_to_string(&marker_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", marker_path.display()));
    let marker = marker.trim();
    assert!(
        !marker.is_empty(),
        "portable runtime marker file is empty: {}",
        marker_path.display()
    );
    println!("cargo:rustc-env=ASTRBOT_PORTABLE_RUNTIME_MARKER={marker}");

    tauri_build::build()
}
