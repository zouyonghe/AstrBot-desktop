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
ASTRBOT_BUILD_SOURCE_DIR ?=
ASTRBOT_RESET_ENV_SCRIPT ?= .astrbot-reset-env.sh
RUST_MANIFEST ?= src-tauri/Cargo.toml
NODE_MODULES_DIR ?= node_modules
PNPM_STORE_DIR ?= .pnpm-store
TAURI_TARGET_DIR ?= src-tauri/target
# Single source of env keys managed by `make clean-env`.
# If build/resource scripts start consuming a new persistent env var, add it here.
ASTRBOT_ENV_KEYS := ASTRBOT_SOURCE_DIR ASTRBOT_SOURCE_GIT_URL ASTRBOT_SOURCE_GIT_REF ASTRBOT_DESKTOP_VERSION ASTRBOT_BUILD_SOURCE_DIR
# Hash of ASTRBOT_ENV_KEYS for stale reset-script detection in `make clean-env`.
ASTRBOT_ENV_KEYS_HASH := $(shell (printf '%s\n' "$(ASTRBOT_ENV_KEYS)" | shasum -a 256 2>/dev/null || printf '%s\n' "$(ASTRBOT_ENV_KEYS)" | sha256sum 2>/dev/null || printf '%s\n' "$(ASTRBOT_ENV_KEYS)" | cksum 2>/dev/null) | awk '{print $$1}' | head -n 1)

.PHONY: help deps sync-version update prepare-webui prepare-backend prepare-resources dev build \
	prepare rebuild lint test doctor prune size clean clean-rust clean-resources \
	clean-vendor-local clean-vendor clean-node clean-env clean-all

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
	@echo "                          (set ASTRBOT_SOURCE_DIR=... or ASTRBOT_BUILD_SOURCE_DIR=...)"
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
	@echo "  make clean-env          Generate shell script to unset build env vars"
	@echo "                          (then source the script in current shell)"
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
	@set -e; \
	build_version="$(ASTRBOT_DESKTOP_VERSION)"; \
	build_source_dir="$(ASTRBOT_BUILD_SOURCE_DIR)"; \
	if [ -z "$$build_source_dir" ]; then \
		build_source_dir="$(ASTRBOT_SOURCE_DIR)"; \
	fi; \
	if [ -z "$$build_version" ]; then \
		build_version="$$(node -e "console.log(require('./package.json').version)")"; \
	fi; \
	if [ -n "$$build_source_dir" ]; then \
		echo "Using build source dir: $$build_source_dir"; \
	fi; \
	echo "Build resource source dir: $${build_source_dir:-<auto vendor from git ref>}"; \
	export ASTRBOT_SOURCE_GIT_URL="$(ASTRBOT_SOURCE_GIT_URL)"; \
	export ASTRBOT_SOURCE_GIT_REF="$(ASTRBOT_SOURCE_GIT_REF)"; \
	export ASTRBOT_DESKTOP_VERSION="$$build_version"; \
	if [ -n "$$build_source_dir" ]; then \
		export ASTRBOT_SOURCE_DIR="$$build_source_dir"; \
	fi; \
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

clean-env:
	@set -e; \
	reset_script="$(ASTRBOT_RESET_ENV_SCRIPT)"; \
	current_hash="$(ASTRBOT_ENV_KEYS_HASH)"; \
	existing_hash=""; \
	if [ -f "$$reset_script" ]; then \
		existing_hash="$$(sed -n 's/^# ASTRBOT_ENV_KEYS_HASH=//p' "$$reset_script" | head -n 1)"; \
	fi; \
	if [ "$$existing_hash" != "$$current_hash" ]; then \
		{ \
			echo "#!/usr/bin/env sh"; \
			echo "# Generated by make clean-env. Keys come from ASTRBOT_ENV_KEYS in Makefile."; \
			echo "# ASTRBOT_ENV_KEYS_HASH=$$current_hash"; \
			for key in $(ASTRBOT_ENV_KEYS); do \
				printf 'unset %s\n' "$$key"; \
			done; \
		} > "$$reset_script"; \
		chmod +x "$$reset_script"; \
		echo "Generated $$reset_script"; \
	else \
		echo "$$reset_script is up to date"; \
	fi; \
	echo "Run: source $$reset_script"; \
	echo "Note: executing $$reset_script directly runs in a child shell and cannot clear parent-shell env."

clean: clean-rust clean-resources clean-vendor clean-node

clean-all: clean
