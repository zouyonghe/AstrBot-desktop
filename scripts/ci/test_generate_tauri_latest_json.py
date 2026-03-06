import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).with_name('generate-tauri-latest-json.py')
SPEC = importlib.util.spec_from_file_location('generate_tauri_latest_json', SCRIPT_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)
FORMAT_SPEC = json.loads(
    (SCRIPT_PATH.parents[1] / '..' / 'src-tauri' / 'nightly-version-format.json').resolve().read_text()
)


class GenerateTauriLatestJsonTests(unittest.TestCase):
    def test_nightly_version_regex_matches_shared_examples(self):
        for raw in FORMAT_SPEC['validExamples']:
            self.assertIsNotNone(MODULE.NIGHTLY_VERSION_RE.fullmatch(raw), raw)

        for raw in FORMAT_SPEC['invalidExamples']:
            self.assertIsNone(MODULE.NIGHTLY_VERSION_RE.fullmatch(raw), raw)

    def test_derive_base_version_removes_nightly_suffix(self):
        self.assertEqual(
            MODULE.derive_base_version('4.29.0-nightly.20260307.abcd1234'),
            '4.29.0',
        )
        self.assertEqual(MODULE.derive_base_version('4.29.0'), '4.29.0')

    def test_normalize_arch_aliases(self):
        self.assertEqual(MODULE.normalize_arch('x86_64'), 'amd64')
        self.assertEqual(MODULE.normalize_arch('x64'), 'amd64')
        self.assertEqual(MODULE.normalize_arch('amd64'), 'amd64')
        self.assertEqual(MODULE.normalize_arch('aarch64'), 'arm64')
        self.assertEqual(MODULE.normalize_arch('arm64'), 'arm64')

    def test_platform_key_for_windows_unsupported_arch(self):
        with self.assertRaisesRegex(ValueError, r'Unsupported Windows arch: ppc64le'):
            MODULE.platform_key_for_windows('ppc64le')

    def test_platform_key_for_macos_unsupported_arch(self):
        with self.assertRaisesRegex(ValueError, r'Unsupported macOS arch: ppc64le'):
            MODULE.platform_key_for_macos('ppc64le')

    def test_derive_nightly_filename_suffix_validation(self):
        self.assertEqual(
            MODULE.derive_nightly_filename_suffix('4.29.0', 'stable'),
            '',
        )
        self.assertEqual(
            MODULE.derive_nightly_filename_suffix(
                '4.29.0-nightly.20260307.abcd1234',
                'nightly',
            ),
            '_nightly_abcd1234',
        )

        invalid_versions = [
            '4.29.0-nightly',
            '4.29.0-nightly.2026-03-07.abcd1234',
            '4.29.0-nightly.20260307.abc',
            'not-a-nightly-version',
        ]
        for raw in invalid_versions:
            with self.subTest(version=raw):
                with self.assertRaisesRegex(ValueError, 'Nightly manifest version must match'):
                    MODULE.derive_nightly_filename_suffix(raw, 'nightly')

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

    def test_collect_platforms_normalizes_legacy_windows_x86_64_alias(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0_windows_x86_64_setup.exe.sig').write_text('sig-win')

            platforms = MODULE.collect_platforms(
                root,
                'AstrBotDevs/AstrBot-desktop',
                'v4.29.0',
                version='4.29.0',
                channel='stable',
            )

        self.assertIn('windows-x86_64', platforms)
        self.assertEqual(
            platforms['windows-x86_64']['url'],
            'https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/'
            'AstrBot_4.29.0_windows_amd64_setup.exe',
        )

    def test_collect_platforms_accepts_stable_macos_arm64_name(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0_macos_arm64.zip.sig').write_text('sig-mac')

            platforms = MODULE.collect_platforms(
                root,
                'AstrBotDevs/AstrBot-desktop',
                'v4.29.0',
                version='4.29.0',
                channel='stable',
            )

        self.assertIn('darwin-aarch64', platforms)
        self.assertEqual(
            platforms['darwin-aarch64']['url'],
            'https://github.com/AstrBotDevs/AstrBot-desktop/releases/download/v4.29.0/'
            'AstrBot_4.29.0_macos_arm64.zip',
        )

    def test_collect_platforms_invalid_windows_sig_raises(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0_windows_amd64.exe.sig').write_text('sig-win')

            with self.assertRaisesRegex(ValueError, 'Unexpected Windows artifact name'):
                MODULE.collect_platforms(
                    root,
                    'AstrBotDevs/AstrBot-desktop',
                    'v4.29.0',
                    version='4.29.0',
                    channel='stable',
                )

    def test_collect_platforms_invalid_macos_sig_raises(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / 'AstrBot_4.29.0_macos_invalidarch.dmg.zip.sig').write_text('sig-mac')

            with self.assertRaisesRegex(ValueError, 'Unexpected macOS artifact name'):
                MODULE.collect_platforms(
                    root,
                    'AstrBotDevs/AstrBot-desktop',
                    'v4.29.0',
                    version='4.29.0',
                    channel='stable',
                )


if __name__ == '__main__':
    unittest.main()
