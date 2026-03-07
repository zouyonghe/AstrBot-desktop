from __future__ import annotations

import re

# Known artifact extensions. Some may overlap (for example a future generic
# ".sig" alongside ".app.tar.gz.sig"), so callers must choose the longest
# matching suffix at call time instead of relying on declaration order.
ARTIFACT_EXTENSIONS: tuple[str, ...] = (
    ".app.tar.gz.sig",
    ".app.tar.gz",
    ".AppImage.sig",
    ".exe.sig",
    ".msi.sig",
    ".zip.sig",
    ".AppImage",
    ".rpm",
    ".deb",
    ".exe",
    ".msi",
    ".zip",
)

MACOS_UPDATER_ARCHIVE_EXTENSION = ".app.tar.gz"
MACOS_UPDATER_SIGNATURE_EXTENSION = f"{MACOS_UPDATER_ARCHIVE_EXTENSION}.sig"
MACOS_UPDATER_ARCHIVE_REGEX_FRAGMENT = re.escape(MACOS_UPDATER_ARCHIVE_EXTENSION)

VERSION_PATTERN = r"[0-9A-Za-z.+-]+"
ARCH_PATTERN = r"[A-Za-z0-9_]+"
LOCALE_PATTERN = r"[A-Za-z0-9-]+"

CANONICAL_VERSION_PATTERN = r"[^_]+"
LEGACY_VERSION_PATTERN = r".+?"
CANONICAL_ARCH_PATTERN = r"[^_]+"
LEGACY_ARCH_PATTERN = r"[^.]+"
SHORT_SHA_PATTERN = r"[0-9a-fA-F]{8}"
CANONICAL_NIGHTLY_SUFFIX_PATTERN = rf"(?:_nightly_{SHORT_SHA_PATTERN})?"

WINDOWS_ARTIFACT_STEM_PATTERN_FRAGMENT = (
    rf"^AstrBot_(?P<version>{VERSION_PATTERN})_(?:windows_)?(?P<arch>{ARCH_PATTERN})"
)
MACOS_CANONICAL_ARTIFACT_STEM_PATTERN = re.compile(
    rf"^AstrBot_(?P<version>{VERSION_PATTERN})_macos_(?P<arch>{ARCH_PATTERN})$"
)

WINDOWS_UPDATER_PATTERNS: tuple[re.Pattern[str], ...] = (
    # Canonical:
    # <name>_<version>_windows_<arch>-setup.exe
    # <name>_<version>_windows_<arch>_setup_nightly_<shortsha>.exe
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{CANONICAL_VERSION_PATTERN})_windows_(?P<arch>{CANONICAL_ARCH_PATTERN})"
        rf"(?:-setup|_setup{CANONICAL_NIGHTLY_SUFFIX_PATTERN})\.exe$"
    ),
    # Legacy:
    # <name>_<version>_<arch>-setup.exe
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{LEGACY_VERSION_PATTERN})_(?P<arch>x64|amd64|arm64)-setup\.exe$"
    ),
)

MACOS_UPDATER_ARCHIVE_PATTERNS: tuple[re.Pattern[str], ...] = (
    # Canonical .app.tar.gz:
    # <name>_<version>_macos_<arch>_nightly_<shortsha>.app.tar.gz
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{CANONICAL_VERSION_PATTERN})_macos_(?P<arch>{CANONICAL_ARCH_PATTERN})"
        rf"{CANONICAL_NIGHTLY_SUFFIX_PATTERN}{MACOS_UPDATER_ARCHIVE_REGEX_FRAGMENT}$"
    ),
    # Legacy .app.tar.gz:
    # <name>_<version>_macos_<arch>.app.tar.gz
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{LEGACY_VERSION_PATTERN})_macos_(?P<arch>{LEGACY_ARCH_PATTERN}){MACOS_UPDATER_ARCHIVE_REGEX_FRAGMENT}$"
    ),
)

LINUX_APPIMAGE_UPDATER_PATTERNS: tuple[re.Pattern[str], ...] = (
    # Canonical:
    # <name>_<version>_linux_<arch>_nightly_<shortsha>.AppImage
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{CANONICAL_VERSION_PATTERN})_linux_(?P<arch>{CANONICAL_ARCH_PATTERN})"
        rf"{CANONICAL_NIGHTLY_SUFFIX_PATTERN}\.AppImage$"
    ),
    # Legacy:
    # <name>_<version>_<arch>.AppImage
    re.compile(
        rf"(?P<name>.+?)_(?P<version>{LEGACY_VERSION_PATTERN})_(?P<arch>x86_64|x64|amd64|aarch64|arm64)\.AppImage$"
    ),
)


class ReleaseArtifactError(RuntimeError):
    pass


def match_any(
    filename: str, patterns: tuple[re.Pattern[str], ...]
) -> re.Match[str] | None:
    for pattern in patterns:
        match = pattern.match(filename)
        if match:
            return match
    return None
