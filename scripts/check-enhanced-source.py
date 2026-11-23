#!/usr/bin/env python3
"""Check Enhanced source publication inputs without building or publishing."""
import argparse
import hashlib
import re
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
REQUIRED = (
    'README.md', 'LICENSE', 'NOTICE.md', 'CHANGELOG.md',
    'CONTRIBUTING.md', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    '.gitattributes', '.github/workflows/source-release.yml',
    'src/update/enhanced_releases.rs', 'ui/enhanced_settings.slint',
    'crates/yapfast-plugin-api/Cargo.toml',
    'crates/yapfast-plugin-host/Cargo.toml',
    'crates/yapfast-plugins/Cargo.toml',
    'tools/yapfast-enhanced-manager/Cargo.toml',
    'tools/yapfast-enhanced-manager/Cargo.lock',
    'pdk/rust/Cargo.toml', 'schemas/yapfast-plugin.schema.json',
    'plugins/ping/yapfast-plugin.json', 'plugins/ping/plugin.wasm',
    'plugins/ping/plugin.wasm.sha256', 'testing/plugins/fixtures.sha256',
    'packaging/windows/yapfast-enhanced.iss',
    'packaging/windows/yapfast-enhanced-universal.iss',
    'scripts/package-enhanced-universal.py', 'docs/universal-installer.md',
    'tools/yapfast-enhanced-manager/src/bootstrap.rs',
    'packaging/windows/branding/yapfast-enhanced.png',
    'scripts/package-enhanced-inplace.py', 'scripts/release_package.py',
    'testing/test_enhanced_release.py', 'docs/releasing-enhanced.md',
    'docs/README.md', 'docs/enhanced-inplace.md', 'docs/enhanced-tools.md',
    'docs/plugins/README.md',
)
PRIVATE_DIRS = {'.git', '.local', '.release', 'target', 'target-manager', 'node_modules', '.venv', '__pycache__'}
PRIVATE_NAMES = {'id_rsa', 'id_ed25519', 'credentials.json', '.env'}
TEST_KEYS = {
    'src/update/testdata/stable.key': '365534606e3fd25fc957b6473467c1fc74501d36',
    'src/update/testdata/nightly.key': '5cd17eb14f07ce92985942545205cda613d18b51',
}


def unsafe_path(name):
    # Upstream signature fixtures use deliberately public test-only keys.
    # They are required to reproduce updater verification tests.
    if name in TEST_KEYS:
        return False
    path = PurePosixPath(name)
    if path.is_absolute() or '..' in path.parts:
        return True
    if any(part in PRIVATE_DIRS for part in path.parts):
        return True
    if path.name in PRIVATE_NAMES or path.suffix.lower() in {'.pfx', '.p12', '.key', '.pyc'}:
        return True
    return path.name.startswith('.env.') and not path.name.endswith('.example')


def git(root, *args):
    # Command-scoped ownership exception supports a read-only sandbox account.
    # No persistent Git configuration, staging or remote access is changed.
    return subprocess.check_output(
        ['git', '-c', f'safe.directory={root.as_posix()}', '-C', str(root), *args],
        timeout=30,
    ).decode('utf-8')


def candidates(root):
    return sorted(set(filter(None, git(root, 'ls-files', '--cached', '--others',
                                      '--exclude-standard', '-z').split('\0'))))


def metadata_errors(root):
    errors = []
    source = (root/'src/update/enhanced_releases.rs').read_text(encoding='utf-8')
    match = re.search(r'pub const VERSION: &str = "((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))";', source)
    if match is None:
        return ['Enhanced VERSION must be four numbers: Yapfast major.minor.patch.Enhanced-revision (revision starts at 0)']
    version = match.group(1)
    if any(int(part) > 65535 for part in version.split('.')):
        errors.append('Enhanced version components exceed the Windows version limit')
    base = tomllib.loads((root/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    if version.rsplit('.', 1)[0] != base:
        errors.append('Enhanced version does not start with the supported Yapfast version')
    # Cargo package versions remain three-part SemVer. Product numbering lives
    # in VERSION and is used directly by the app, installer and release tags.
    if f'Enhanced v{version}' not in (root/'README.md').read_text(encoding='utf-8'):
        errors.append('README does not describe the current Enhanced version')
    if f'## {version} - ' not in (root/'CHANGELOG.md').read_text(encoding='utf-8'):
        errors.append('CHANGELOG has no entry for the Enhanced version')
    if 'api.github.com/repos/dmor-me/Yapfast-Enhanced/releases' not in source:
        errors.append('Enhanced checker points outside its intended release repository')
    return errors


def check(root, require_clean=False):
    errors = []
    names = candidates(root)
    present = set(names)
    for name in REQUIRED:
        if name not in present or not (root/name).is_file():
            errors.append(f'Required release source missing or ignored: {name}')
    for name in names:
        if unsafe_path(name):
            errors.append(f'Private/generated file would enter source publication: {name}')
        if name in TEST_KEYS:
            # Pin only the exact, already tracked upstream public test fixture.
            # A replacement key at the same path must never inherit the exception.
            data=(root/name).read_bytes().replace(b'\r\n', b'\n')
            blob=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
            if blob != TEST_KEYS[name]:
                errors.append(f'Test signing fixture differs from its public upstream pin: {name}')
    # Check local Cargo paths without resolving or downloading any dependencies.
    for name in names:
        if not name.endswith('Cargo.toml') or name.startswith('vendor/'):
            continue
        path = root/name
        if not path.is_file():
            continue
        manifest = tomllib.loads(path.read_text(encoding='utf-8'))
        def local_paths(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key == 'path' and isinstance(child, str):
                        yield child
                    else:
                        yield from local_paths(child)
        for relative in local_paths(manifest):
            # Cargo path dependencies name directories; lib/bin source paths name files.
            if relative.endswith('.rs'):
                continue
            dependency = (path.parent/relative).resolve()
            try:
                dependency.relative_to(root.resolve())
            except ValueError:
                errors.append(f'Cargo path outside release source: {name}')
                continue
            if not (dependency/'Cargo.toml').is_file():
                errors.append(f'Cargo path dependency missing: {name}: {relative}')
            elif (dependency/'Cargo.toml').relative_to(root.resolve()).as_posix() not in present:
                errors.append(f'Cargo dependency source is ignored: {name}: {relative}')
    if not any(name.startswith('crates/yapfast-plugin-host/') for name in names):
        errors.append('Runtime helper source is absent')
    if not errors:
        errors.extend(metadata_errors(root))
    dirty = bool(git(root, 'status', '--porcelain', '--untracked-files=normal').strip())
    if require_clean and dirty:
        errors.append('Commit the reviewed source before tagging or packaging a public release')
    return names, dirty, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require-clean', action='store_true',
                        help='also refuse uncommitted source before tagging or packaging')
    args = parser.parse_args()
    try:
        names, dirty, errors = check(ROOT, args.require_clean)
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print(f'Source checks could not complete: {type(error).__name__}', file=sys.stderr)
        return 1
    for error in errors:
        print(f'FAIL: {error}', file=sys.stderr)
    if errors:
        return 1
    print(f'Enhanced source checks passed ({len(names)} publication candidates).')
    if dirty:
        print('Working-tree review only. Commit reviewed changes before a public tag/build.')
    print('No build, staging, tag, push or release publication performed.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
