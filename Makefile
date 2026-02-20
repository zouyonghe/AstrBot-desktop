.DEFAULT_GOAL := help

VENDOR_DIR ?= vendor
RUNTIME_DIR ?= runtime
RESOURCES_DIR ?= resources
RESOURCES_BACKEND_DIR ?= $(RESOURCES_DIR)/backend
RESOURCES_WEBUI_DIR ?= $(RESOURCES_DIR)/webui
ASTRBOT_LOCAL_DIR ?= $(VENDOR_DIR)/AstrBot-local
ASTRBOT_LOCAL_DESKTOP_DIR ?= $(ASTRBOT_LOCAL_DIR)/desktop
ASTRBOT_SOURCE_GIT_URL ?= https://github.com/AstrBotDevs/AstrBot.git
ASTRBOT_SOURCE_GIT_REF ?= master
RUST_MANIFEST ?= src-tauri/Cargo.toml
NODE_MODULES_DIR ?= node_modules
PNPM_STORE_DIR ?= .pnpm-store
TAURI_TARGET_DIR ?= src-tauri/target

.PHONY: help deps sync-version update prepare-webui prepare-backend prepare-resources dev build \
	prepare rebuild lint test doctor prune size clean clean-rust clean-resources \
	clean-vendor-local clean-vendor clean-node clean-all

help:
	@echo "AstrBot Desktop Make Targets"
	@echo ""
	@echo "  make deps               Install JS dependencies"
	@echo "  make sync-version       Sync desktop version from AstrBot source"
	@echo "  make update             Sync desktop version from upstream AstrBot"
	@echo "  make prepare            Alias of prepare-resources"
	@echo "  make prepare-webui      Build and sync WebUI resources"
	@echo "  make prepare-backend    Build and sync backend runtime resources"
	@echo "  make prepare-resources  Prepare all resources"
	@echo "  make dev                Run Tauri dev"
	@echo "  make build              Run Tauri build"
	@echo "  make rebuild            Clean and build"
	@echo "  make lint               Run formatting and clippy checks"
	@echo "  make test               Run Rust tests"
	@echo "  make doctor             Show local toolchain versions"
	@echo "  make prune              Remove heavy local runtime caches"
	@echo ""
	@echo "  make size               Show disk usage of heavy directories"
	@echo "  make clean-rust         Clean Rust build outputs"
	@echo "  make clean-resources    Clean generated resources"
	@echo "  make clean-vendor-local Remove vendor/AstrBot-local"
	@echo "  make clean-vendor       Remove vendor and runtime"
	@echo "  make clean-node         Remove node_modules and pnpm store"
	@echo "  make clean              Clean all build artifacts"
	@echo "  make clean-all          Alias of clean"

deps:
	pnpm install

sync-version:
	pnpm run sync:version

update:
	ASTRBOT_SOURCE_DIR= \
	ASTRBOT_SOURCE_GIT_URL=$(ASTRBOT_SOURCE_GIT_URL) \
	ASTRBOT_SOURCE_GIT_REF=$(ASTRBOT_SOURCE_GIT_REF) \
	ASTRBOT_DESKTOP_VERSION=$(ASTRBOT_DESKTOP_VERSION) \
	pnpm run sync:version

prepare-webui:
	pnpm run prepare:webui

prepare-backend:
	pnpm run prepare:backend

prepare-resources:
	pnpm run prepare:resources

prepare: prepare-resources

dev:
	pnpm run dev

build:
	pnpm run build

rebuild: clean build

lint:
	cargo fmt --manifest-path $(RUST_MANIFEST) --all -- --check
	cargo clippy --manifest-path $(RUST_MANIFEST) --locked --all-targets -- -D warnings

test:
	cargo test --manifest-path $(RUST_MANIFEST) --locked

doctor:
	@echo "node:  $$(node -v)"
	@echo "pnpm:  $$(pnpm -v)"
	@echo "rustc: $$(rustc -V)"
	@echo "cargo: $$(cargo -V)"

prune:
	rm -rf $(ASTRBOT_LOCAL_DIR) $(RUNTIME_DIR)

size:
	@echo "== project size =="
	@du -sh $(VENDOR_DIR) $(RUNTIME_DIR) $(RESOURCES_DIR) $(TAURI_TARGET_DIR) 2>/dev/null || true
	@echo ""
	@echo "== vendor top =="
	@du -sh $(VENDOR_DIR)/* 2>/dev/null | sort -h | tail -n 20 || true
	@echo ""
	@echo "== $(ASTRBOT_LOCAL_DESKTOP_DIR) top =="
	@du -sh $(ASTRBOT_LOCAL_DESKTOP_DIR)/* 2>/dev/null | sort -h | tail -n 20 || true

clean-rust:
	cargo clean --manifest-path $(RUST_MANIFEST)

clean-resources:
	rm -rf $(RESOURCES_BACKEND_DIR) $(RESOURCES_WEBUI_DIR)

clean-vendor-local:
	rm -rf $(ASTRBOT_LOCAL_DIR)

clean-vendor:
	rm -rf $(VENDOR_DIR) $(RUNTIME_DIR)

clean-node:
	rm -rf $(NODE_MODULES_DIR) $(PNPM_STORE_DIR)

clean: clean-rust clean-resources clean-vendor clean-node

clean-all: clean
