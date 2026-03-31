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


if __name__ == "__main__":
    unittest.main()
