from __future__ import annotations

ARCH_ALIAS = {
    "x86_64": "amd64",
    "x64": "amd64",
    "amd64": "amd64",
    "aarch64": "arm64",
    "arm64": "arm64",
}


def normalize_arch_alias(arch: str) -> str | None:
    return ARCH_ALIAS.get(arch)
