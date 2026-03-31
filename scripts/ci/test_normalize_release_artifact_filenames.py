import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

from scripts.ci import normalize_release_artifact_filenames as MODULE


SCRIPT_PATH = Path(MODULE.__file__)


class NormalizeReleaseArtifactFilenamesTests(unittest.TestCase):
    def test_main_normalizes_legacy_windows_portable_zip_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            legacy_name = "AstrBot_4.29.0_x64-portable.zip"
            legacy_path = root / legacy_name
            legacy_path.write_text("portable")

            stdout = io.StringIO()
            stderr = io.StringIO()
            argv = [
                str(SCRIPT_PATH),
                "--root",
                str(root),
            ]
            with mock.patch("sys.argv", argv):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    exit_code = MODULE.main()

            self.assertEqual(exit_code, 0)
            self.assertFalse(legacy_path.exists())
            self.assertTrue(
                (root / "AstrBot_4.29.0_windows_amd64_portable.zip").exists()
            )

    def test_main_normalizes_canonical_windows_portable_zip_with_nightly_suffix(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            original_name = "AstrBot_4.29.0_windows_x64_portable_nightly_deadbeef.zip"
            original_path = root / original_name
            original_path.write_text("portable-nightly")

            stdout = io.StringIO()
            stderr = io.StringIO()
            argv = [
                str(SCRIPT_PATH),
                "--root",
                str(root),
            ]
            with mock.patch("sys.argv", argv):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    exit_code = MODULE.main()

            self.assertEqual(exit_code, 0)
            self.assertFalse(original_path.exists())
            self.assertTrue(
                (
                    root / "AstrBot_4.29.0_windows_amd64_portable_nightly_deadbeef.zip"
                ).exists()
            )
            self.assertNotIn("None", stdout.getvalue())
            self.assertNotIn("None", stderr.getvalue())
            for path in root.iterdir():
                self.assertNotIn("None", path.name)

    def test_main_normalizes_canonical_windows_portable_zip_without_nightly_suffix(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            original_name = "AstrBot_4.29.0_windows_x64_portable.zip"
            original_path = root / original_name
            original_path.write_text("portable")

            stdout = io.StringIO()
            stderr = io.StringIO()
            argv = [
                str(SCRIPT_PATH),
                "--root",
                str(root),
            ]
            with mock.patch("sys.argv", argv):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    exit_code = MODULE.main()

            self.assertEqual(exit_code, 0)
            self.assertFalse(original_path.exists())
            self.assertTrue(
                (root / "AstrBot_4.29.0_windows_amd64_portable.zip").exists()
            )

    def test_main_normalizes_legacy_nightly_windows_setup_signature(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            original_name = "AstrBot_4.19.2-nightly.20260306.7ac169c5_x64-setup.exe.sig"
            original_path = root / original_name
            original_path.write_text("windows-signature")

            stdout = io.StringIO()
            stderr = io.StringIO()
            argv = [
                str(SCRIPT_PATH),
                "--root",
                str(root),
                "--build-mode",
                "nightly",
                "--source-git-ref",
                "7ac169c5e81cee0acc1416d22d7ee4464a507a8d",
            ]
            with mock.patch("sys.argv", argv):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    exit_code = MODULE.main()

            self.assertEqual(exit_code, 0)
            self.assertFalse(original_path.exists())
            self.assertTrue(
                (
                    root / "AstrBot_4.19.2_windows_amd64_setup_nightly_7ac169c5.exe.sig"
                ).exists()
            )

    def test_main_normalizes_legacy_nightly_linux_appimage(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            original_name = "AstrBot_4.19.2-nightly.20260306.7ac169c5_aarch64.AppImage"
            original_path = root / original_name
            original_path.write_text("appimage")

            stdout = io.StringIO()
            stderr = io.StringIO()
            argv = [
                str(SCRIPT_PATH),
                "--root",
                str(root),
                "--build-mode",
                "nightly",
                "--source-git-ref",
                "7ac169c5e81cee0acc1416d22d7ee4464a507a8d",
            ]
            with mock.patch("sys.argv", argv):
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    exit_code = MODULE.main()

            self.assertEqual(exit_code, 0)
            self.assertFalse(original_path.exists())
            self.assertTrue(
                (root / "AstrBot_4.19.2_linux_arm64_nightly_7ac169c5.AppImage").exists()
            )
            self.assertNotIn("None", stdout.getvalue())
            self.assertNotIn("None", stderr.getvalue())
            for path in root.iterdir():
                self.assertNotIn("None", path.name)


if __name__ == "__main__":
    unittest.main()
