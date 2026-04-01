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

    def test_installer_to_portable_name_accepts_canonical_windows_arm64_name(self):
        self.assertEqual(
            MODULE.installer_to_portable_name("AstrBot_4.29.0_windows_arm64_setup.exe"),
            "AstrBot_4.29.0_windows_arm64_portable.zip",
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

    def test_installer_to_portable_name_normalizes_legacy_windows_aarch64_name(self):
        self.assertEqual(
            MODULE.installer_to_portable_name("AstrBot_4.29.0_aarch64-setup.exe"),
            "AstrBot_4.29.0_windows_arm64_portable.zip",
        )

    def test_installer_to_portable_name_rejects_unexpected_name(self):
        with self.assertRaisesRegex(ValueError, "Unexpected Windows installer name"):
            MODULE.installer_to_portable_name("AstrBot-setup.exe")

    def test_resolve_release_dir_uses_bundle_parent(self):
        bundle_dir = Path(
            "/tmp/project/src-tauri/target/aarch64-pc-windows-msvc/release/bundle/nsis"
        )

        self.assertEqual(
            MODULE.resolve_release_dir(bundle_dir),
            Path("/tmp/project/src-tauri/target/aarch64-pc-windows-msvc/release"),
        )

    def test_resolve_project_root_from_finds_anchor_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            script_path = (
                project_root / "scripts" / "ci" / "package_windows_portable.py"
            )
            tauri_config_path = project_root / "src-tauri" / "tauri.conf.json"

            script_path.parent.mkdir(parents=True)
            script_path.write_text("# placeholder")
            tauri_config_path.parent.mkdir(parents=True)
            tauri_config_path.write_text('{"productName":"AstrBot"}')

            self.assertEqual(
                MODULE.resolve_project_root_from(script_path), project_root.resolve()
            )

    def test_resolve_project_root_from_rejects_missing_anchor_files(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            script_path = (
                Path(tmpdir) / "scripts" / "ci" / "package_windows_portable.py"
            )
            script_path.parent.mkdir(parents=True)
            script_path.write_text("# placeholder")

            with self.assertRaisesRegex(
                FileNotFoundError, "Unable to locate project root"
            ):
                MODULE.resolve_project_root_from(script_path)

    def test_load_portable_runtime_marker_reads_shared_marker_file(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            marker_path = project_root / MODULE.PORTABLE_RUNTIME_MARKER_RELATIVE_PATH
            marker_path.parent.mkdir(parents=True)
            marker_path.write_text("portable.flag\n")

            self.assertEqual(
                MODULE.load_portable_runtime_marker(project_root),
                "portable.flag",
            )

    def test_iter_installer_paths_only_returns_installer_style_executables(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            bundle_dir = Path(tmpdir)
            canonical = bundle_dir / "AstrBot_4.29.0_windows_amd64_setup.exe"
            legacy = bundle_dir / "AstrBot_4.29.0_x64-setup.exe"
            helper = bundle_dir / "helper.exe"
            updater = bundle_dir / "AstrBot_4.29.0_windows_amd64_updater.exe"
            canonical.write_text("installer")
            legacy.write_text("legacy")
            helper.write_text("helper")
            updater.write_text("updater")

            self.assertEqual(
                MODULE.iter_installer_paths(bundle_dir),
                [canonical, legacy],
            )

    def test_populate_portable_root_copies_release_bundle_contents(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            bundle_dir = (
                project_root / "src-tauri" / "target" / "release" / "bundle" / "nsis"
            )
            release_dir = project_root / "src-tauri" / "target" / "release"
            destination_root = project_root / "portable"
            backend_dir = project_root / "resources" / "backend"
            webui_dir = project_root / "resources" / "webui"
            windows_dir = project_root / "src-tauri" / "windows"
            tauri_config_path = project_root / "src-tauri" / "tauri.conf.json"

            bundle_dir.mkdir(parents=True)
            release_dir.mkdir(parents=True, exist_ok=True)
            backend_dir.mkdir(parents=True)
            webui_dir.mkdir(parents=True)
            windows_dir.mkdir(parents=True)

            tauri_config_path.write_text('{"productName":"AstrBot"}')
            (release_dir / "AstrBot.exe").write_text("exe")
            (release_dir / "WebView2Loader.dll").write_text("dll")
            (backend_dir / "runtime-manifest.json").write_text("{}")
            (backend_dir / "launch_backend.py").write_text("print('ok')")
            (webui_dir / "index.html").write_text("<html></html>")
            (windows_dir / "kill-backend-processes.ps1").write_text(
                "Write-Host cleanup"
            )

            MODULE.populate_portable_root(
                bundle_dir=bundle_dir,
                destination_root=destination_root,
                project_root=project_root,
            )

            self.assertTrue((destination_root / "AstrBot.exe").is_file())
            self.assertTrue((destination_root / "WebView2Loader.dll").is_file())
            self.assertTrue(
                (
                    destination_root / "resources" / "backend" / "runtime-manifest.json"
                ).is_file()
            )
            self.assertTrue(
                (destination_root / "resources" / "webui" / "index.html").is_file()
            )
            self.assertTrue((destination_root / "kill-backend-processes.ps1").is_file())
            self.assertTrue((destination_root / MODULE.PORTABLE_MARKER_NAME).is_file())
            self.assertTrue((destination_root / MODULE.PORTABLE_README_NAME).is_file())

    def test_populate_portable_root_rejects_missing_main_executable(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            bundle_dir = (
                project_root / "src-tauri" / "target" / "release" / "bundle" / "nsis"
            )
            destination_root = project_root / "portable"
            backend_dir = project_root / "resources" / "backend"
            webui_dir = project_root / "resources" / "webui"
            tauri_config_path = project_root / "src-tauri" / "tauri.conf.json"

            bundle_dir.mkdir(parents=True)
            backend_dir.mkdir(parents=True)
            webui_dir.mkdir(parents=True)
            tauri_config_path.parent.mkdir(parents=True, exist_ok=True)
            tauri_config_path.write_text('{"productName":"AstrBot"}')
            (backend_dir / "runtime-manifest.json").write_text("{}")
            (webui_dir / "index.html").write_text("<html></html>")

            with self.assertRaisesRegex(FileNotFoundError, "Main executable not found"):
                MODULE.populate_portable_root(
                    bundle_dir=bundle_dir,
                    destination_root=destination_root,
                    project_root=project_root,
                )

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
