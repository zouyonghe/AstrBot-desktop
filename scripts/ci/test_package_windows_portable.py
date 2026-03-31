import tempfile
import unittest
from pathlib import Path

from scripts.ci import package_windows_portable as MODULE


class PackageWindowsPortableTests(unittest.TestCase):
    def test_installer_to_portable_name_accepts_canonical_windows_name(self):
        self.assertEqual(
            MODULE.installer_to_portable_name("AstrBot_4.29.0_windows_amd64_setup.exe"),
            "AstrBot_4.29.0_windows_amd64_portable.zip",
        )

    def test_installer_to_portable_name_accepts_canonical_nightly_windows_name(self):
        self.assertEqual(
            MODULE.installer_to_portable_name(
                "AstrBot_4.29.0_windows_x64_setup_nightly_deadbeef.exe"
            ),
            "AstrBot_4.29.0_windows_amd64_portable_nightly_deadbeef.zip",
        )

    def test_installer_to_portable_name_rejects_noncanonical_nightly_suffix_length(
        self,
    ):
        with self.assertRaisesRegex(ValueError, "Unexpected Windows installer name"):
            MODULE.installer_to_portable_name(
                "AstrBot_4.29.0_windows_x64_setup_nightly_deadbeef12.exe"
            )

    def test_installer_to_portable_name_normalizes_legacy_windows_name(self):
        self.assertEqual(
            MODULE.installer_to_portable_name("AstrBot_4.29.0_x64-setup.exe"),
            "AstrBot_4.29.0_windows_amd64_portable.zip",
        )

    def test_installer_to_portable_name_rejects_unexpected_name(self):
        with self.assertRaisesRegex(ValueError, "Unexpected Windows installer name"):
            MODULE.installer_to_portable_name("AstrBot-setup.exe")

    def test_find_nsis_payload_archive_prefers_app_prefixed_archives(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            plugin_dir = root / "$PLUGINSDIR"
            plugin_dir.mkdir()
            fallback_archive = root / "payload.7z"
            preferred_archive = plugin_dir / "app-64.7z"
            fallback_archive.write_text("fallback")
            preferred_archive.write_text("preferred")

            self.assertEqual(
                MODULE.find_nsis_payload_archive(root),
                preferred_archive,
            )

    def test_find_nsis_payload_archive_rejects_missing_archives(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)

            with self.assertRaisesRegex(FileNotFoundError, "embedded .7z payload"):
                MODULE.find_nsis_payload_archive(root)

    def test_find_nsis_payload_archive_rejects_multiple_preferred_archives(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            plugin_dir = root / "$PLUGINSDIR"
            plugin_dir.mkdir()
            (plugin_dir / "app-64.7z").write_text("x64")
            (plugin_dir / "app-32.7z").write_text("x86")

            with self.assertRaisesRegex(
                RuntimeError, "Expected exactly one NSIS payload archive"
            ):
                MODULE.find_nsis_payload_archive(root)

    def test_find_nsis_payload_archive_rejects_multiple_fallback_archives(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "payload-a.7z").write_text("a")
            (root / "payload-b.7z").write_text("b")

            with self.assertRaisesRegex(
                RuntimeError, "Expected exactly one NSIS payload archive"
            ):
                MODULE.find_nsis_payload_archive(root)

    def test_select_payload_root_collapses_single_top_level_directory(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            portable_root = root / "AstrBot"
            portable_root.mkdir()
            (portable_root / "AstrBot.exe").write_text("binary")

            self.assertEqual(MODULE.select_payload_root(root), portable_root)

    def test_add_portable_runtime_files_writes_marker_and_readme(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)

            MODULE.add_portable_runtime_files(root)

            self.assertTrue((root / MODULE.PORTABLE_MARKER_NAME).is_file())
            self.assertIn(
                "manual updates",
                (root / MODULE.PORTABLE_README_NAME).read_text().lower(),
            )

    def test_validate_portable_root_accepts_expected_layout(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot.exe").write_text("binary")
            (root / "resources" / "backend").mkdir(parents=True)
            (root / "resources" / "webui").mkdir(parents=True)
            (root / "resources" / "backend" / "runtime-manifest.json").write_text("{}")
            (root / "resources" / "webui" / "index.html").write_text("<html></html>")

            MODULE.validate_portable_root(root)

    def test_validate_portable_root_requires_expected_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot.exe").write_text("binary")

            with self.assertRaisesRegex(ValueError, "runtime-manifest.json"):
                MODULE.validate_portable_root(root)

    def test_validate_portable_root_requires_top_level_exe(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "resources" / "backend").mkdir(parents=True)
            (root / "resources" / "webui").mkdir(parents=True)
            (root / "resources" / "backend" / "runtime-manifest.json").write_text("{}")
            (root / "resources" / "webui" / "index.html").write_text("<html></html>")

            with self.assertRaisesRegex(ValueError, r"top-level \*\.exe"):
                MODULE.validate_portable_root(root)


if __name__ == "__main__":
    unittest.main()
