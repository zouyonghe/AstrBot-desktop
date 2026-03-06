import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name('generate-tauri-latest-json.py')
SPEC = importlib.util.spec_from_file_location('generate_tauri_latest_json', SCRIPT_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class GenerateTauriLatestJsonTests(unittest.TestCase):
    def test_derive_base_version_removes_nightly_suffix(self):
        self.assertEqual(
            MODULE.derive_base_version('4.29.0-nightly.20260307.abcd1234'),
            '4.29.0',
        )
        self.assertEqual(MODULE.derive_base_version('4.29.0'), '4.29.0')

    def test_build_payload_includes_channel_metadata(self):
        payload = MODULE.build_payload(
            version='4.29.0-nightly.20260307.abcd1234',
            notes='nightly build',
            channel='nightly',
            base_version='4.29.0',
            release_tag='nightly',
            platforms={
                'darwin-aarch64': {
                    'signature': 'sig',
                    'url': 'https://example.com/AstrBot_4.29.0_macos_arm64.zip',
                }
            },
        )

        self.assertEqual(payload['channel'], 'nightly')
        self.assertEqual(payload['baseVersion'], '4.29.0')
        self.assertEqual(payload['releaseTag'], 'nightly')

    def test_collect_platforms_normalizes_nightly_release_filenames(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0-nightly.20260307.abcd1234_x64-setup.exe.sig').write_text('sig-win')
            (root / 'AstrBot_4.29.0-nightly.20260307.abcd1234_macos_aarch64.zip.sig').write_text('sig-mac')

            platforms = MODULE.collect_platforms(
                root,
                'AstrBotDevs/AstrBot-desktop',
                'nightly',
                version='4.29.0-nightly.20260307.abcd1234',
                channel='nightly',
            )

        self.assertEqual(
            platforms['windows-x86_64']['url'],
            'https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/'
            'AstrBot_4.29.0_windows_amd64_setup_nightly_abcd1234.exe',
        )
        self.assertEqual(
            platforms['darwin-aarch64']['url'],
            'https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/nightly/'
            'AstrBot_4.29.0_macos_arm64_nightly_abcd1234.zip',
        )

    def test_collect_platforms_accepts_current_canonical_stable_windows_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0_windows_amd64_setup.exe.sig').write_text('sig-win')

            platforms = MODULE.collect_platforms(
                root,
                'AstrBotDevs/AstrBot-desktop',
                'v4.29.0',
                version='4.29.0',
                channel='stable',
            )

        self.assertEqual(
            platforms['windows-x86_64']['url'],
            'https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/'
            'AstrBot_4.29.0_windows_amd64_setup.exe',
        )


if __name__ == '__main__':
    unittest.main()
