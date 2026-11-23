"""Regression checks for complete source packaging and release guards."""
import importlib.util
import tempfile
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT/relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SourceReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.guard = load('enhanced_source_checks', 'scripts/check-enhanced-source.py')
        cls.installer = load('enhanced_installer_package', 'scripts/package-enhanced-inplace.py')
        cls.package = load('release_source_package', 'scripts/release_package.py')

    def test_private_and_nested_build_inputs_are_refused(self):
        for name in ['.local/profile/settings.json', '.release/installer.exe',
                     'plugins/ping/target/release/plugin.wasm',
                     'pdk/assemblyscript/node_modules/a.js', '.env', '.env.production',
                     'signing/private.key', 'signing/certificate.pfx', '../token']:
            with self.subTest(name=name):
                self.assertTrue(self.guard.unsafe_path(name))

    def test_required_binary_fixtures_and_examples_remain_publishable(self):
        for name in ['.env.example', '.envrc', 'plugins/ping/plugin.wasm',
                     'testing/plugins/trap.wasm', 'packaging/windows/branding/header.bmp',
                     'src/update/testdata/stable.key', 'src/update/testdata/nightly.key',
                     '.github/workflows/source-release.yml', 'tools/yapfast-enhanced-manager/Cargo.lock']:
            with self.subTest(name=name):
                self.assertFalse(self.guard.unsafe_path(name))

    def test_source_packager_covers_enhanced_build_inputs(self):
        for directory in ['pdk', 'plugins', 'schemas', 'tools', 'marketplace', '.github']:
            self.assertIn(directory, self.package.SOURCE_DIRECTORIES)
        for name in ['NOTICE.md', 'CHANGELOG.md', 'CONTRIBUTING.md', '.gitattributes']:
            self.assertIn(name, self.package.SOURCE_FILES)

    def test_source_snapshot_includes_plugin_sdk_manager_and_fixture_bytes(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name)
            repo=root/'repo'
            repo.mkdir()
            names=['pdk/rust/Cargo.toml', 'tools/yapfast-enhanced-manager/Cargo.lock',
                   'schemas/yapfast-plugin.schema.json', 'plugins/ping/plugin.wasm']
            for relative in names:
                file=repo/relative
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b'\0asm' if relative.endswith('.wasm') else b'fixture\n')
            def output(args, **kwargs):
                return ('\0'.join(names)+'\0').encode() if 'ls-files' in args else b'1700000000\n'
            import zipfile
            with patch.object(self.package, 'ROOT', repo), patch.object(self.package.subprocess, 'check_output', output):
                self.assertFalse(self.package.source_snapshot({'packages':[]}, 'Enhanced', root, root/'source.zip'))
            with zipfile.ZipFile(root/'source.zip') as archive:
                self.assertEqual(set(archive.namelist()), {'Enhanced-source/'+n for n in names})
                self.assertEqual(archive.read('Enhanced-source/plugins/ping/plugin.wasm'), b'\0asm')

    def test_four_part_product_version_and_base_validation(self):
        self.assertEqual(self.installer.enhanced_version(), '0.3.0.0')
        self.installer.validate_base('0.3.0.0', '0.3.0')
        with self.assertRaises(SystemExit):
            self.installer.validate_base('0.3.0.0', '0.5.0')
        with tempfile.TemporaryDirectory() as name:
            root=Path(name)
            source=root/'src/update/enhanced_releases.rs'
            source.parent.mkdir(parents=True)
            for version in ['0.3.0', '0.3.0.01', '0.3.0E1', '0.3.0.65536', '0.3.0.0-beta']:
                source.write_text(f'pub const VERSION: &str = "{version}";\n', encoding='utf-8')
                with self.subTest(version=version), patch.object(self.installer, 'ROOT', root):
                    with self.assertRaises(SystemExit):
                        self.installer.enhanced_version()
            source.write_text('pub const VERSION: &str = "0.3.0.65535";\n', encoding='utf-8')
            with patch.object(self.installer, 'ROOT', root):
                self.assertEqual(self.installer.enhanced_version(), '0.3.0.65535')

    def test_release_guard_rejects_wrong_base_and_old_product_format(self):
        with tempfile.TemporaryDirectory() as name:
            root=Path(name)
            source=root/'src/update/enhanced_releases.rs'
            source.parent.mkdir(parents=True)
            for file in ['Cargo.toml', 'README.md', 'CHANGELOG.md']:
                (root/file).write_text((ROOT/file).read_text(encoding='utf-8'), encoding='utf-8')
            text=(ROOT/'src/update/enhanced_releases.rs').read_text(encoding='utf-8')
            source.write_text(text.replace('"0.3.0.0"', '"0.5.0.1"'), encoding='utf-8')
            self.assertTrue(any('supported Yapfast' in error for error in self.guard.metadata_errors(root)))
            source.write_text(text.replace('"0.3.0.0"', '"0.1.0"'), encoding='utf-8')
            self.assertTrue(any('four numbers' in error for error in self.guard.metadata_errors(root)))

    def test_installer_rejects_development_and_mismatched_binaries(self):
        for summary in ['Yapfast v0.3.0 · Development build · 30d1bc8d',
                        'Yapfast v0.4.0 · Stable · 03862f63',
                        'Yapfast v0.3.0 · Nightly', 'Yapfast v0.3.0 · Stable-ish', '']:
            completed=subprocess.CompletedProcess([], 0, stdout=summary)
            with self.subTest(summary=summary), patch.object(self.installer.subprocess, 'run', return_value=completed):
                with self.assertRaisesRegex(SystemExit, 'YAPFAST_RELEASE=1'):
                    self.installer.verify_release_binary(Path('yapfast.exe'), '0.3.0')
        completed=subprocess.CompletedProcess([], 0, stdout='Yapfast v0.3.0 · Stable · abcdef01 · 2026-11-22\n')
        with patch.object(self.installer.subprocess, 'run', return_value=completed) as execute:
            self.assertIn('Stable', self.installer.verify_release_binary(Path('yapfast.exe'), '0.3.0'))
            self.assertEqual(execute.call_args.args[0], ['yapfast.exe', '--version'])

    def test_installer_requires_the_matching_enhanced_product_version(self):
        base = self.installer.enhanced_version().rsplit('.', 1)[0]
        version = self.installer.enhanced_version()
        for product, accepted in [(f'Enhanced {version}', True), ('Enhanced 99.0.0.0', False)]:
            replies = [subprocess.CompletedProcess([], 0, stdout=f'Yapfast v{base} \u00b7 Stable'),
                       subprocess.CompletedProcess([], 0, stdout=product)]
            with patch.object(self.installer.subprocess, 'run', side_effect=replies):
                if accepted:
                    self.installer.verify_release_binary(Path('app.exe'), base, version)
                else:
                    with self.assertRaisesRegex(SystemExit, 'version mismatch'):
                        self.installer.verify_release_binary(Path('app.exe'), base, version)

    def test_installer_refuses_unreadable_build_metadata(self):
        for error in [OSError('missing app'), subprocess.TimeoutExpired('yapfast', 20),
                      subprocess.CalledProcessError(1, 'yapfast')]:
            with self.subTest(error=error), patch.object(self.installer.subprocess, 'run', side_effect=error):
                with self.assertRaisesRegex(SystemExit, 'Could not verify'):
                    self.installer.verify_release_binary(Path('yapfast.exe'), '0.3.0')

    def test_current_release_versions_are_consistent(self):
        self.assertEqual(self.guard.metadata_errors(ROOT), [])


if __name__ == '__main__':
    unittest.main()
