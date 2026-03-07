import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from scripts.ci import generate_tauri_latest_json as MODULE


SCRIPT_PATH = Path(MODULE.__file__)
FORMAT_SPEC = json.loads(
    (SCRIPT_PATH.parents[1] / ".." / "src-tauri" / "nightly-version-format.json")
    .resolve()
    .read_text()
)


class GenerateTauriLatestJsonTests(unittest.TestCase):
    def test_nightly_canonical_format_matches_shared_spec(self):
        self.assertEqual(
            MODULE.NIGHTLY_CANONICAL_FORMAT, FORMAT_SPEC["canonicalFormat"]
        )

    def test_nightly_version_regex_matches_shared_examples(self):
        for raw in FORMAT_SPEC["validExamples"]:
            self.assertIsNotNone(MODULE.NIGHTLY_VERSION_RE.fullmatch(raw), raw)

        for raw in FORMAT_SPEC["invalidExamples"]:
            self.assertIsNone(MODULE.NIGHTLY_VERSION_RE.fullmatch(raw), raw)

    def test_normalize_arch_aliases(self):
        self.assertEqual(MODULE.normalize_arch("x86_64"), "amd64")
        self.assertEqual(MODULE.normalize_arch("x64"), "amd64")
        self.assertEqual(MODULE.normalize_arch("amd64"), "amd64")
        self.assertEqual(MODULE.normalize_arch("aarch64"), "arm64")
        self.assertEqual(MODULE.normalize_arch("arm64"), "arm64")

    def test_platform_key_for_windows_unsupported_arch(self):
        with self.assertRaisesRegex(ValueError, r"Unsupported Windows arch: ppc64le"):
            MODULE.platform_key_for_windows("ppc64le")

    def test_platform_key_for_macos_unsupported_arch(self):
        with self.assertRaisesRegex(ValueError, r"Unsupported macOS arch: ppc64le"):
            MODULE.platform_key_for_macos("ppc64le")

    def test_platform_key_for_linux_appimage_unsupported_arch(self):
        with self.assertRaisesRegex(
            ValueError, r"Unsupported Linux AppImage arch: ppc64le"
        ):
            MODULE.platform_key_for_linux_appimage("ppc64le")

    def test_derive_release_metadata_validates_and_returns_expected_values(self):
        self.assertEqual(
            MODULE.derive_release_metadata("4.29.0", None),
            ("stable", "4.29.0", ""),
        )
        self.assertEqual(
            MODULE.derive_release_metadata("4.29.0-nightly.20260307.abcd1234", None),
            ("nightly", "4.29.0", "_nightly_abcd1234"),
        )
        self.assertEqual(
            MODULE.derive_release_metadata(
                "4.29.0-nightly.20260307.abcd1234",
                "nightly",
            ),
            ("nightly", "4.29.0", "_nightly_abcd1234"),
        )

        invalid_versions = [
            "4.29.0-nightly",
            "4.29.0-nightly.2026-03-07.abcd1234",
            "4.29.0-nightly.20260307.abc",
            "not-a-nightly-version",
        ]
        for raw in invalid_versions:
            with self.subTest(version=raw):
                with self.assertRaisesRegex(
                    ValueError, "Nightly manifest version must match"
                ):
                    MODULE.derive_release_metadata(raw, "nightly")

    def test_derive_release_metadata_error_mentions_sha8(self):
        with self.assertRaisesRegex(ValueError, r"<sha8"):
            MODULE.derive_release_metadata("4.29.0-nightly", "nightly")

    def test_derive_release_metadata_rejects_malformed_inferred_nightly(self):
        with self.assertRaisesRegex(ValueError, "Invalid nightly version"):
            MODULE.derive_release_metadata("4.29.0-nightly-beta", None)

    def test_canonical_windows_filename_outputs_expected_names(self):
        self.assertEqual(
            MODULE.canonical_windows_filename("AstrBot", "amd64", "4.29.0", "stable"),
            "AstrBot_4.29.0_windows_amd64_setup.exe",
        )
        self.assertEqual(
            MODULE.canonical_windows_filename(
                "AstrBot",
                "amd64",
                "4.29.0-nightly.20260307.abcd1234",
                "nightly",
            ),
            "AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe",
        )

    def test_canonical_macos_filename_outputs_expected_names(self):
        self.assertEqual(
            MODULE.canonical_macos_filename(
                "AstrBot",
                "arm64",
                "4.29.0",
                "stable",
            ),
            "AstrBot_4.29.0_macos_arm64.app.tar.gz",
        )
        self.assertEqual(
            MODULE.canonical_macos_filename(
                "AstrBot",
                "arm64",
                "4.29.0-nightly.20260307.abcd1234",
                "nightly",
            ),
            "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz",
        )

    def test_canonical_linux_appimage_filename_outputs_expected_names(self):
        self.assertEqual(
            MODULE.canonical_linux_appimage_filename(
                "AstrBot", "arm64", "4.29.0", "stable"
            ),
            "AstrBot_4.29.0_linux_arm64.AppImage",
        )
        self.assertEqual(
            MODULE.canonical_linux_appimage_filename(
                "AstrBot",
                "aarch64",
                "4.29.0-nightly.20260307.abcd1234",
                "nightly",
            ),
            "AstrBot_4.29.0_linux_arm64_nightly_abcd1234.AppImage",
        )

    def test_main_writes_expected_manifest_json(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            output = root / "latest-nightly.json"
            (
                root / "AstrBot_4.29.0-nightly.20260307.abcd1234_x64-setup.exe.sig"
            ).write_text("sig-win")

            argv = [
                str(SCRIPT_PATH),
                "--artifacts-root",
                str(root),
                "--repo",
                "AstrBotDevs/AstrBot-desktop",
                "--tag",
                "nightly",
                "--version",
                "4.29.0-nightly.20260307.abcd1234",
                "--channel",
                "nightly",
                "--output",
                str(output),
                "--notes",
                "nightly build",
            ]

            with mock.patch("sys.argv", argv):
                exit_code = MODULE.main()

            payload = json.loads(output.read_text())

        self.assertEqual(exit_code, 0)
        self.assertEqual(payload["version"], "4.29.0-nightly.20260307.abcd1234")
        self.assertEqual(payload["notes"], "nightly build")
        self.assertEqual(payload["channel"], "nightly")
        self.assertEqual(payload["baseVersion"], "4.29.0")
        self.assertEqual(payload["releaseTag"], "nightly")
        self.assertIn("platforms", payload)
        self.assertEqual(
            payload["platforms"]["windows-x86_64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe",
        )
        self.assertEqual(
            payload["platforms"]["windows-x86_64"]["signature"],
            "sig-win",
        )

    def test_main_fails_when_no_signatures_found(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            output = root / "latest-stable.json"
            argv = [
                str(SCRIPT_PATH),
                "--artifacts-root",
                str(root),
                "--repo",
                "AstrBotDevs/AstrBot-desktop",
                "--tag",
                "v4.29.0",
                "--version",
                "4.29.0",
                "--channel",
                "stable",
                "--output",
                str(output),
            ]

            with mock.patch("sys.argv", argv):
                with self.assertRaisesRegex(
                    SystemExit, "No updater signatures found under artifacts root"
                ):
                    MODULE.main()

            self.assertFalse(output.exists())

    def test_main_fails_when_inferred_nightly_version_is_malformed(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            output = root / "latest-nightly.json"
            argv = [
                str(SCRIPT_PATH),
                "--artifacts-root",
                str(root),
                "--repo",
                "AstrBotDevs/AstrBot-desktop",
                "--tag",
                "nightly",
                "--version",
                "4.29.0-nightly-beta",
                "--output",
                str(output),
            ]

            with mock.patch("sys.argv", argv):
                with self.assertRaisesRegex(SystemExit, "Invalid nightly version"):
                    MODULE.main()

            self.assertFalse(output.exists())

    def test_collect_platforms_preserves_canonical_nightly_filenames(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (
                root / "AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe.sig"
            ).write_text("sig-win")
            (
                root / "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz.sig"
            ).write_text("sig-mac")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "nightly",
                version="4.29.0-nightly.20260307.abcd1234",
                channel="nightly",
            )

        self.assertEqual(
            platforms["windows-x86_64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe",
        )
        self.assertEqual(
            platforms["darwin-aarch64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz",
        )

    def test_collect_platforms_normalizes_nightly_release_filenames(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (
                root / "AstrBot_4.29.0-nightly.20260307.abcd1234_x64-setup.exe.sig"
            ).write_text("sig-win")
            (
                root
                / "AstrBot_4.29.0-nightly.20260307.abcd1234_macos_aarch64.app.tar.gz.sig"
            ).write_text("sig-mac")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "nightly",
                version="4.29.0-nightly.20260307.abcd1234",
                channel="nightly",
            )

        self.assertEqual(
            platforms["windows-x86_64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe",
        )
        self.assertEqual(
            platforms["darwin-aarch64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz",
        )

    def test_collect_platforms_accepts_current_canonical_stable_windows_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_windows_amd64_setup.exe.sig").write_text("sig-win")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "v4.29.0",
                version="4.29.0",
                channel="stable",
            )

        self.assertEqual(
            platforms["windows-x86_64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/"
            "AstrBot_4.29.0_windows_amd64_setup.exe",
        )

    def test_collect_platforms_normalizes_legacy_windows_x86_64_alias(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_windows_x86_64_setup.exe.sig").write_text("sig-win")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "v4.29.0",
                version="4.29.0",
                channel="stable",
            )

        self.assertIn("windows-x86_64", platforms)
        self.assertEqual(
            platforms["windows-x86_64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/"
            "AstrBot_4.29.0_windows_amd64_setup.exe",
        )

    def test_collect_platforms_accepts_stable_macos_arm64_app_tar_gz(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.app.tar.gz.sig").write_text(
                "sig-mac-app"
            )

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "v4.29.0",
                version="4.29.0",
                channel="stable",
            )

        self.assertIn("darwin-aarch64", platforms)
        self.assertEqual(
            platforms["darwin-aarch64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/"
            "AstrBot_4.29.0_macos_arm64.app.tar.gz",
        )

    def test_collect_platforms_accepts_nightly_macos_arm64_app_tar_gz_canonical_name(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (
                root / "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz.sig"
            ).write_text("sig-mac-nightly")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "nightly",
                version="4.29.0-nightly.20260307.abcd1234",
                channel="nightly",
            )

        self.assertIn("darwin-aarch64", platforms)
        self.assertEqual(
            platforms["darwin-aarch64"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_macos_arm64_nightly_abcd1234.app.tar.gz",
        )

    def test_collect_platforms_rejects_macos_zip_signature_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.zip.sig").write_text("sig-mac")

            with self.assertRaisesRegex(
                ValueError,
                "Unsupported updater signature files under artifacts root",
            ):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_rejects_macos_zip_signature_files_even_with_valid_sig(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.app.tar.gz.sig").write_text(
                "sig-mac-valid"
            )
            (root / "AstrBot_4.29.0_macos_arm64.zip.sig").write_text("sig-mac-invalid")

            with self.assertRaisesRegex(
                ValueError,
                "Unsupported updater signature files under artifacts root",
            ):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_ignores_non_artifact_sig_files_even_with_valid_sig(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.app.tar.gz.sig").write_text(
                "sig-mac-valid"
            )
            (root / "unexpected.sig").write_text("sig-unknown")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "v4.29.0",
                version="4.29.0",
                channel="stable",
            )

        self.assertIn("darwin-aarch64", platforms)

    def test_collect_platforms_error_lists_all_unsupported_signature_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.zip.sig").write_text("sig-mac-invalid")
            (root / "AstrBot_4.29.0_windows_amd64_setup.msi.sig").write_text(
                "sig-win-invalid"
            )

            with self.assertRaises(ValueError) as cm:
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

            error_message = str(cm.exception)
            self.assertRegex(error_message, r"AstrBot_4\.29\.0_macos_arm64\.zip\.sig")
            self.assertRegex(
                error_message, r"AstrBot_4\.29\.0_windows_amd64_setup\.msi\.sig"
            )

    def test_collect_platforms_ignores_non_artifact_sig_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_arm64.app.tar.gz.sig").write_text(
                "sig-mac-valid"
            )
            (root / "notes.sig").write_text("not-an-artifact-signature")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "v4.29.0",
                version="4.29.0",
                channel="stable",
            )

        self.assertIn("darwin-aarch64", platforms)

    def test_collect_platforms_accepts_linux_appimage_canonical_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (
                root / "AstrBot_4.29.0_linux_arm64_nightly_abcd1234.AppImage.sig"
            ).write_text("sig-linux")

            platforms = MODULE.collect_platforms(
                root,
                "AstrBotDevs/AstrBot-desktop",
                "nightly",
                version="4.29.0-nightly.20260307.abcd1234",
                channel="nightly",
            )

        self.assertIn("linux-aarch64-appimage", platforms)
        self.assertEqual(
            platforms["linux-aarch64-appimage"]["url"],
            "https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/"
            "AstrBot_4.29.0_linux_arm64_nightly_abcd1234.AppImage",
        )

    def test_collect_platforms_invalid_windows_sig_raises(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_windows_amd64.exe.sig").write_text("sig-win")

            with self.assertRaisesRegex(
                ValueError,
                r"Expected format: <name>_<version>_windows_<arch>_setup\.exe",
            ):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_invalid_macos_sig_raises(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_macos_.app.tar.gz.sig").write_text("sig-mac")

            with self.assertRaisesRegex(ValueError, "Unexpected macOS artifact name"):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_rejects_macos_name_without_macos_prefix(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_arm64.app.tar.gz.sig").write_text("sig-mac")

            with self.assertRaisesRegex(ValueError, "Unexpected macOS artifact name"):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_rejects_duplicate_artifacts(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_windows_amd64_setup.exe.sig").write_text(
                "sig-win-1"
            )
            (root / "AstrBot_4.29.0_x64-setup.exe.sig").write_text("sig-win-2")

            with self.assertRaisesRegex(
                ValueError, r"Duplicate .* artifact for platform"
            ):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )

    def test_collect_platforms_rejects_unsupported_signature_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot_4.29.0_windows_amd64_setup.msi.sig").write_text(
                "sig-unknown"
            )

            with self.assertRaisesRegex(
                ValueError,
                "Unsupported updater signature files under artifacts root",
            ):
                MODULE.collect_platforms(
                    root,
                    "AstrBotDevs/AstrBot-desktop",
                    "v4.29.0",
                    version="4.29.0",
                    channel="stable",
                )


if __name__ == "__main__":
    unittest.main()
