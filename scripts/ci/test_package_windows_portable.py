import json
import re
import tempfile
import unittest
from pathlib import Path

from scripts.ci import package_windows_portable as MODULE

PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH = MODULE.PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH
PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH = MODULE.PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH


class PackageWindowsPortableTests(unittest.TestCase):
    def make_project_layout(
        self,
        *,
        product_name: str = "AstrBot",
        cargo_toml: str = '[package]\nname = "astrbot-desktop-tauri"\n',
        marker_name: str = "portable.flag\n",
    ) -> dict[str, Path]:
        project_root = Path(self.enterContext(tempfile.TemporaryDirectory()))
        script_path = project_root / "scripts" / "ci" / "package_windows_portable.py"
        tauri_config_path = project_root / "src-tauri" / "tauri.conf.json"
        cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
        marker_path = project_root / MODULE.PORTABLE_RUNTIME_MARKER_RELATIVE_PATH

        script_path.parent.mkdir(parents=True)
        script_path.write_text("# placeholder")
        tauri_config_path.parent.mkdir(parents=True)
        tauri_config_path.write_text(json.dumps({"productName": product_name}))
        cargo_toml_path.write_text(cargo_toml)
        marker_path.parent.mkdir(parents=True, exist_ok=True)
        marker_path.write_text(marker_name)

        return {
            "project_root": project_root,
            "script_path": script_path,
            "tauri_config_path": tauri_config_path,
            "cargo_toml_path": cargo_toml_path,
            "marker_path": marker_path,
        }

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

    def test_installer_to_portable_name_normalizes_legacy_nightly_windows_name(self):
        arch_cases = [
            ("x64", "amd64"),
            ("amd64", "amd64"),
            ("arm64", "arm64"),
            ("aarch64", "arm64"),
        ]
        separators = [".", "_", "-"]

        for arch_input, arch_output in arch_cases:
            for separator in separators:
                with self.subTest(arch=arch_input, separator=separator):
                    installer_name = f"AstrBot_4.29.0-nightly{separator}20260401{separator}deadbeef_{arch_input}-setup.exe"
                    expected_name = f"AstrBot_4.29.0_windows_{arch_output}_portable_nightly_deadbeef.zip"
                    self.assertEqual(
                        MODULE.installer_to_portable_name(installer_name),
                        expected_name,
                    )

    def test_installer_to_portable_name_treats_malformed_legacy_nightly_as_non_nightly(
        self,
    ):
        arch_cases = [
            ("x64", "amd64"),
            ("amd64", "amd64"),
            ("arm64", "arm64"),
            ("aarch64", "arm64"),
        ]
        separators = [".", "_", "-"]
        malformed_fragments = [
            ("nightly{sep}20260401{sep}deadbee", "short_sha"),
            ("nightly{sep}20261301{sep}deadbeef", "invalid_date"),
            ("nightly{sep}20260401", "missing_sha"),
            ("nightly{sep}20260401{sep}deadbeef{sep}extra", "extra_component"),
        ]

        for arch_input, arch_output in arch_cases:
            for separator in separators:
                for fragment_template, case_name in malformed_fragments:
                    fragment = fragment_template.format(sep=separator)
                    with self.subTest(
                        arch=arch_input,
                        separator=separator,
                        malformed_case=case_name,
                    ):
                        installer_name = (
                            f"AstrBot_4.29.0-{fragment}_{arch_input}-setup.exe"
                        )
                        expected_name = (
                            f"AstrBot_4.29.0_windows_{arch_output}_portable.zip"
                        )
                        self.assertEqual(
                            MODULE.installer_to_portable_name(installer_name),
                            expected_name,
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

    def test_load_project_config_from_returns_root_product_and_marker(self):
        layout = self.make_project_layout()

        project_config = MODULE.load_project_config_from(layout["script_path"])

        self.assertEqual(project_config.root, layout["project_root"].resolve())
        self.assertEqual(project_config.product_name, "AstrBot")
        self.assertEqual(project_config.binary_name, "astrbot-desktop-tauri")
        self.assertEqual(project_config.portable_marker_name, "portable.flag")

    def test_normalize_legacy_nightly_version_returns_base_version_and_suffix(self):
        self.assertEqual(
            MODULE.normalize_legacy_nightly_version("4.29.0-nightly.20260401.deadbeef"),
            ("4.29.0", "_nightly_deadbeef"),
        )

    def test_normalize_legacy_nightly_version_strips_malformed_nightly_suffix(self):
        self.assertEqual(
            MODULE.normalize_legacy_nightly_version("4.29.0-nightly-20260401"),
            ("4.29.0", ""),
        )

    def test_load_project_config_from_rejects_product_name_with_invalid_windows_chars(
        self,
    ):
        invalid_product_names = [
            "AstrBot<Dev>",
            "AstrBot>Dev",
            'AstrBot:"Test"',
            "AstrBot/Dev",
            r"AstrBot\Dev",
            "AstrBot|Beta",
            "AstrBot?",
            "AstrBot*",
        ]

        for product_name in invalid_product_names:
            with self.subTest(product_name=product_name):
                layout = self.make_project_layout(product_name=product_name)

                with self.assertRaisesRegex(ValueError, "invalid Windows filename"):
                    MODULE.load_project_config_from(layout["script_path"])

    def test_load_project_config_from_rejects_reserved_windows_device_names(self):
        reserved_product_names = ["CON", "NUL", "PRN", "COM1", "LPT9"]

        for product_name in reserved_product_names:
            with self.subTest(product_name=product_name):
                layout = self.make_project_layout(product_name=product_name)

                with self.assertRaisesRegex(ValueError, "reserved device name"):
                    MODULE.load_project_config_from(layout["script_path"])

    def test_load_project_config_from_rejects_product_name_with_trailing_dot(self):
        layout = self.make_project_layout(product_name="AstrBot.")

        with self.assertRaisesRegex(ValueError, "trailing spaces or dots"):
            MODULE.load_project_config_from(layout["script_path"])

    def test_load_project_config_from_strips_exe_suffix_from_product_name(self):
        for raw_name in ("AstrBot.exe", "AstrBot.EXE", "AstrBot.ExE", "AstrBot.exe   "):
            with self.subTest(product_name=raw_name):
                layout = self.make_project_layout(product_name=raw_name)

                project_config = MODULE.load_project_config_from(layout["script_path"])

                self.assertEqual(project_config.product_name, "AstrBot")

    def test_load_cargo_package_name_supports_inline_comments(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text(
                '[package]\nname = "astrbot-desktop-tauri" # main binary\n'
            )

            self.assertEqual(
                MODULE.load_binary_name_from_cargo(project_root),
                "astrbot-desktop-tauri",
            )

    def test_load_cargo_package_name_missing_cargo_toml_raises_file_not_found(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"

            with self.assertRaisesRegex(
                FileNotFoundError, re.escape(str(cargo_toml_path))
            ):
                MODULE.load_binary_name_from_cargo(project_root)

    def test_load_cargo_package_name_missing_package_table_raises_value_error(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text('[workspace]\nmembers = ["crates/*"]\n')

            with self.assertRaisesRegex(ValueError, re.escape(str(cargo_toml_path))):
                MODULE.load_binary_name_from_cargo(project_root)

    def test_load_cargo_package_name_missing_package_name_raises_value_error(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text('[package]\nversion = "0.1.0"\n')

            with self.assertRaisesRegex(ValueError, re.escape(str(cargo_toml_path))):
                MODULE.load_binary_name_from_cargo(project_root)

    def test_load_cargo_package_name_empty_package_name_raises_value_error(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text('[package]\nname = ""\n')

            with self.assertRaisesRegex(ValueError, re.escape(str(cargo_toml_path))):
                MODULE.load_binary_name_from_cargo(project_root)

    def test_load_cargo_package_name_falls_back_to_package_when_bin_missing_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text(
                "[package]\n"
                'name = "astrbot-desktop-tauri"\n\n'
                "[[bin]]\n"
                'path = "src/main.rs"\n'
            )

            self.assertEqual(
                MODULE.load_binary_name_from_cargo(project_root),
                "astrbot-desktop-tauri",
            )

    def test_load_cargo_package_name_prefers_explicit_bin_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            cargo_toml_path = project_root / "src-tauri" / "Cargo.toml"
            cargo_toml_path.parent.mkdir(parents=True)
            cargo_toml_path.write_text(
                "[package]\n"
                'name = "astrbot-desktop-tauri"\n\n'
                "[[bin]]\n"
                'name = "AstrBot"\n'
            )

            self.assertEqual(
                MODULE.load_binary_name_from_cargo(project_root), "AstrBot"
            )

    def test_resolve_main_executable_path_uses_binary_name_not_product_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            project_root = Path(tmpdir)
            bundle_dir = (
                project_root / "src-tauri" / "target" / "release" / "bundle" / "nsis"
            )
            release_dir = project_root / "src-tauri" / "target" / "release"
            bundle_dir.mkdir(parents=True)
            release_dir.mkdir(parents=True, exist_ok=True)
            (release_dir / "astrbot-desktop-tauri.exe").write_text("exe")

            project_config = MODULE.ProjectConfig(
                root=project_root,
                product_name="AstrBot",
                binary_name="astrbot-desktop-tauri",
                portable_marker_name="portable.flag",
            )

            self.assertEqual(
                MODULE.resolve_main_executable_path(bundle_dir, project_config),
                release_dir / "astrbot-desktop-tauri.exe",
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
        layout = self.make_project_layout()
        project_root = layout["project_root"]
        script_path = layout["script_path"]
        bundle_dir = (
            project_root / "src-tauri" / "target" / "release" / "bundle" / "nsis"
        )
        release_dir = project_root / "src-tauri" / "target" / "release"
        destination_root = project_root / "portable"
        backend_dir = project_root / "resources" / "backend"
        webui_dir = project_root / "resources" / "webui"
        windows_dir = project_root / "src-tauri" / "windows"

        bundle_dir.mkdir(parents=True)
        release_dir.mkdir(parents=True, exist_ok=True)
        backend_dir.mkdir(parents=True)
        webui_dir.mkdir(parents=True)
        windows_dir.mkdir(parents=True, exist_ok=True)
        (release_dir / "astrbot-desktop-tauri.exe").write_text("exe")
        (release_dir / "WebView2Loader.dll").write_text("dll")
        (backend_dir / "runtime-manifest.json").write_text("{}")
        (backend_dir / "launch_backend.py").write_text("print('ok')")
        (webui_dir / "index.html").write_text("<html></html>")
        (windows_dir / "kill-backend-processes.ps1").write_text("Write-Host cleanup")

        project_config = MODULE.load_project_config_from(script_path)

        MODULE.populate_portable_root(
            bundle_dir=bundle_dir,
            destination_root=destination_root,
            project_config=project_config,
        )

        executable_name = f"{project_config.product_name}.exe"

        self.assertTrue((destination_root / executable_name).is_file())
        self.assertFalse((destination_root / "astrbot-desktop-tauri.exe").exists())
        self.assertTrue((destination_root / "WebView2Loader.dll").is_file())
        self.assertTrue(
            (
                destination_root
                / PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH
                / "runtime-manifest.json"
            ).is_file()
        )
        self.assertTrue(
            (destination_root / PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH / "index.html").is_file()
        )
        self.assertFalse((destination_root / "resources" / "backend").exists())
        self.assertFalse((destination_root / "resources" / "webui").exists())
        self.assertTrue((destination_root / "kill-backend-processes.ps1").is_file())
        self.assertTrue((destination_root / "portable.flag").is_file())
        self.assertTrue((destination_root / MODULE.PORTABLE_README_NAME).is_file())

    def test_populate_portable_root_rejects_missing_main_executable(self):
        layout = self.make_project_layout()
        project_root = layout["project_root"]
        script_path = layout["script_path"]
        bundle_dir = (
            project_root / "src-tauri" / "target" / "release" / "bundle" / "nsis"
        )
        destination_root = project_root / "portable"
        backend_dir = project_root / "resources" / "backend"
        webui_dir = project_root / "resources" / "webui"

        bundle_dir.mkdir(parents=True)
        backend_dir.mkdir(parents=True)
        webui_dir.mkdir(parents=True)
        (backend_dir / "runtime-manifest.json").write_text("{}")
        (webui_dir / "index.html").write_text("<html></html>")

        with self.assertRaisesRegex(FileNotFoundError, "Main executable not found"):
            MODULE.populate_portable_root(
                bundle_dir=bundle_dir,
                destination_root=destination_root,
                project_config=MODULE.load_project_config_from(script_path),
            )

    def test_add_portable_runtime_files_writes_marker_and_readme(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            project_config = MODULE.ProjectConfig(
                root=Path(tmpdir),
                product_name="AstrBot",
                binary_name="astrbot-desktop-tauri",
                portable_marker_name="portable.flag",
            )

            MODULE.add_portable_runtime_files(root, project_config)

            self.assertTrue((root / project_config.portable_marker_name).is_file())
            self.assertIn(
                "manual updates",
                (root / MODULE.PORTABLE_README_NAME).read_text().lower(),
            )

    def test_validate_portable_root_accepts_expected_layout(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "AstrBot.exe").write_text("binary")
            (root / PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH).mkdir(parents=True)
            (root / PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH).mkdir(parents=True)
            (
                root / PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH / "runtime-manifest.json"
            ).write_text("{}")
            (root / PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH / "index.html").write_text(
                "<html></html>"
            )

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
            (root / PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH).mkdir(parents=True)
            (root / PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH).mkdir(parents=True)
            (
                root / PORTABLE_BACKEND_LAYOUT_RELATIVE_PATH / "runtime-manifest.json"
            ).write_text("{}")
            (root / PORTABLE_WEBUI_LAYOUT_RELATIVE_PATH / "index.html").write_text(
                "<html></html>"
            )

            with self.assertRaisesRegex(ValueError, r"top-level \*\.exe"):
                MODULE.validate_portable_root(root)


if __name__ == "__main__":
    unittest.main()
