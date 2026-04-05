use serde_json::Value;
use std::{
    fs,
    path::{Component, Path},
};

const TAURI_CONFIG_PATH: &str = "tauri.conf.json";
const BACKEND_RESOURCE_SOURCE: &str = "../resources/backend";
const WEBUI_RESOURCE_SOURCE: &str = "../resources/webui";

fn load_bundle_resource_alias(tauri_config: &Value, source_relative_path: &str) -> String {
    let resources = tauri_config
        .get("bundle")
        .and_then(|bundle| bundle.get("resources"))
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing bundle.resources table in {TAURI_CONFIG_PATH}"));

    let alias = resources
        .get(source_relative_path)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_else(|| {
            panic!(
                "missing bundle.resources alias for {} in {}",
                source_relative_path, TAURI_CONFIG_PATH
            )
        });
    assert!(
        !alias.is_empty(),
        "bundle.resources alias for {} is empty in {}",
        source_relative_path,
        TAURI_CONFIG_PATH
    );

    let alias_path = Path::new(alias);
    assert!(
        !alias_path.is_absolute()
            && alias_path.components().all(|component| {
                !matches!(
                    component,
                    Component::CurDir
                        | Component::ParentDir
                        | Component::Prefix(_)
                        | Component::RootDir
                )
            }),
        "bundle.resources alias for {} must be a relative path without traversal in {}: {}",
        source_relative_path,
        TAURI_CONFIG_PATH,
        alias
    );

    alias.to_string()
}

fn main() {
    let marker_path = Path::new("windows").join("portable-runtime-marker.txt");
    let tauri_config_path = Path::new(TAURI_CONFIG_PATH);
    println!("cargo:rerun-if-changed={}", marker_path.display());
    println!("cargo:rerun-if-changed={}", tauri_config_path.display());

    let marker = fs::read_to_string(&marker_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", marker_path.display()));
    let marker = marker.trim();
    assert!(
        !marker.is_empty(),
        "portable runtime marker file is empty: {}",
        marker_path.display()
    );
    println!("cargo:rustc-env=ASTRBOT_PORTABLE_RUNTIME_MARKER={marker}");

    let tauri_config_text = fs::read_to_string(tauri_config_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", tauri_config_path.display()));
    let tauri_config: Value = serde_json::from_str(&tauri_config_text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", tauri_config_path.display()));

    let backend_resource_alias = load_bundle_resource_alias(&tauri_config, BACKEND_RESOURCE_SOURCE);
    let webui_resource_alias = load_bundle_resource_alias(&tauri_config, WEBUI_RESOURCE_SOURCE);
    println!("cargo:rustc-env=ASTRBOT_BACKEND_RESOURCE_ALIAS={backend_resource_alias}");
    println!("cargo:rustc-env=ASTRBOT_WEBUI_RESOURCE_ALIAS={webui_resource_alias}");

    tauri_build::build()
}
