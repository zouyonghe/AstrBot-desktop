SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: help deps sync-version prepare-webui prepare-backend prepare-resources dev build \
	prepare rebuild lint test doctor prune size clean clean-rust clean-resources \
	clean-vendor-local clean-vendor clean-node clean-all

help:
	@echo "AstrBot Desktop Make Targets"
	@echo ""
	@echo "  make deps               Install JS dependencies"
	@echo "  make sync-version       Sync desktop version from AstrBot source"
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
	cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
	cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings

test:
	cargo test --manifest-path src-tauri/Cargo.toml --locked

doctor:
	@echo "node:  $$(node -v)"
	@echo "pnpm:  $$(pnpm -v)"
	@echo "rustc: $$(rustc -V)"
	@echo "cargo: $$(cargo -V)"

prune:
	rm -rf vendor/AstrBot-local runtime

size:
	@echo "== project size =="
	@du -sh vendor runtime resources src-tauri/target 2>/dev/null || true
	@echo ""
	@echo "== vendor top =="
	@du -sh vendor/* 2>/dev/null | sort -h | tail -n 20 || true
	@echo ""
	@echo "== vendor/AstrBot-local/desktop top =="
	@du -sh vendor/AstrBot-local/desktop/* 2>/dev/null | sort -h | tail -n 20 || true

clean-rust:
	cargo clean --manifest-path src-tauri/Cargo.toml

clean-resources:
	rm -rf resources/backend resources/webui
	mkdir -p resources/backend resources/webui
	touch resources/backend/.gitkeep resources/webui/.gitkeep

clean-vendor-local:
	rm -rf vendor/AstrBot-local

clean-vendor:
	rm -rf vendor runtime

clean-node:
	rm -rf node_modules .pnpm-store

clean: clean-rust clean-resources clean-vendor clean-node

clean-all: clean
