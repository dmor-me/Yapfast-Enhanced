#!/usr/bin/env python3
"""Package a version-matched in-place Yapfast mod, never an arbitrary injector."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SOURCE_REPOSITORY = 'https://github.com/dmor-me/Yapfast-Enhanced'


def enhanced_version():
    source = (ROOT / 'src/update/enhanced_releases.rs').read_text(encoding='utf-8')
    match = re.search(r'^pub const VERSION: &str = "([^\"]+)";', source, re.MULTILINE)
    if match is None:
        raise SystemExit('Could not read the Enhanced version from src/update/enhanced_releases.rs')
    version = match.group(1)
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise SystemExit('Enhanced version must be four numbers: Yapfast major.minor.patch.Enhanced-revision (revision starts at 0)')
    if any(int(part) > 65535 for part in version.split('.')):
        raise SystemExit('Enhanced version components must fit Windows version metadata (0–65535)')
    return version


def validate_base(version, app_version):
    if version.rsplit('.', 1)[0] != app_version:
        raise SystemExit('Enhanced version must begin with its supported official Yapfast version')



def source_reference():
    try:
        commit = subprocess.run(
            ['git', 'rev-parse', 'HEAD'], cwd=ROOT, check=True,
            capture_output=True, text=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=ROOT,
            check=True, capture_output=True, text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return f'Source repository: {SOURCE_REPOSITORY}\nSource revision: unavailable in this source archive\n'
    return (
        f'Source repository: {SOURCE_REPOSITORY}\n'
        f'Source revision: {SOURCE_REPOSITORY}/tree/{commit}\n'
        f'Source checkout: {"uncommitted changes present" if dirty else "clean"}\n'
    )


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def executable(path):
    with path.open('rb') as stream:
        header = stream.read(64)
        if len(header) != 64 or header[:2] != b'MZ':
            raise SystemExit(f'Not a Windows executable: {path}')
        stream.seek(struct.unpack_from('<I', header, 60)[0])
        if stream.read(6) != b'PE\0\0\x64\x86':
            raise SystemExit(f'Not an x64 Windows executable: {path}')



def verify_release_binary(path, app_version, expected_enhanced=None):
    """Reject an optimized-but-development build before creating a release."""
    try:
        result = subprocess.run(
            [str(path), '--version'], check=True, capture_output=True,
            text=True, encoding='utf-8', timeout=20,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise SystemExit(f'Could not verify the app release metadata: {error}') from error
    summary = result.stdout.strip()
    if summary.split(' · ')[:2] != [f'Yapfast v{app_version}', 'Stable']:
        raise SystemExit(
            f'Refusing to package a non-stable or mismatched app: {summary!r}. '
            'Rebuild the app with YAPFAST_RELEASE=1 as well as cargo --release.'
        )
    if expected_enhanced is not None:
        product = subprocess.run(
            [str(path), '--enhanced-version'], check=True, capture_output=True,
            text=True, encoding='utf-8', timeout=20,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0,
        ).stdout.strip()
        if product != f'Enhanced {expected_enhanced}':
            raise SystemExit(f'Enhanced executable version mismatch: {product!r}')
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream-exe', type=Path, required=True)
    parser.add_argument('--upstream-sha256', required=True)
    parser.add_argument('--app-version', required=True, help='supported official Yapfast version')
    parser.add_argument('--bin-dir', type=Path, default=ROOT / 'target/release')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/dist/in-place')
    parser.add_argument('--no-installer', action='store_true')
    args = parser.parse_args()
    enhanced = enhanced_version()
    validate_base(enhanced, args.app_version)
    expected = args.upstream_sha256.lower()
    if len(expected) != 64 or any(char not in '0123456789abcdef' for char in expected):
        raise SystemExit('An exact trusted official executable SHA-256 is required')
    executable(args.upstream_exe)
    if digest(args.upstream_exe) != expected:
        raise SystemExit('Official input does not match the pinned fingerprint')
    for name in ['yapfast.exe', 'yapfast-plugins.exe', 'yapfast-enhanced-manager.exe']:
        executable(args.bin_dir / name)
    verify_release_binary(args.bin_dir / 'yapfast.exe', args.app_version, enhanced)
    output = args.output.resolve()
    payload = output / 'bundle/payload'
    payload.mkdir(parents=True, exist_ok=True)
    for name in ['yapfast.exe', 'yapfast-plugins.exe']:
        shutil.copy2(args.bin_dir / name, payload / name)
    shutil.copy2(args.bin_dir / 'yapfast-enhanced-manager.exe', payload.parent / 'manager.exe')
    manifest = {
        'schema': 1, 'app_version': args.app_version,
        'upstream_sha256': expected,
        'patched_sha256': digest(payload / 'yapfast.exe'),
        'helper_sha256': digest(payload / 'yapfast-plugins.exe'),
    }
    (payload / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(ROOT / 'LICENSE', payload.parent / 'LICENSE.txt')
    (payload.parent / 'SOURCE.txt').write_text(
        'Yapfast Enhanced in-place plugin modification\n'
        'This is a version-specific plugin-enabled executable replacement, not a universal binary injector.\n'
        f'Official base: Yapfast {args.app_version}, SHA-256 {expected}\n'
        f'Enhanced version: {enhanced}\n'
        + source_reference()
        + 'Plugin reference: blockgameentity/yapfast feat/plugins at cb31511cb33337925a777fec9c05e52399634924\n'
        'License: GPL-3.0-or-later. This Enhanced package is unsigned.\n', encoding='utf-8')
    sample = payload.parent / 'examples/Ping'
    sample.mkdir(parents=True, exist_ok=True)
    for name in ['plugin.wasm', 'yapfast-plugin.json', 'README.md']:
        shutil.copy2(ROOT / 'plugins/ping' / name, sample / name)
    archive = output / f'yapfast-enhanced-for-{expected}.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zipped:
        for name in ['manifest.json', 'yapfast.exe', 'yapfast-plugins.exe']:
            zipped.write(payload / name, name)
        for name in ['LICENSE.txt', 'SOURCE.txt']:
            zipped.write(payload.parent / name, name)
    results = [archive]
    if not args.no_installer:
        compiler = Path(os.environ['LOCALAPPDATA']) / 'Programs/Inno Setup 6/ISCC.exe'
        subprocess.run([str(compiler), f'/DAppVersion={args.app_version}',
                        f'/DEnhancedVersion={enhanced}',
                        f'/DBundleDir={payload.parent}', f'/O{output}',
                        str(ROOT / 'packaging/windows/yapfast-enhanced.iss')], check=True)
        results.append(output / 'Yapfast-Enhanced-Installer.exe')
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{digest(file)}  {file.name}\n' for file in results), encoding='ascii')
    print(json.dumps({'installer': str(results[-1]), 'official_sha256': expected,
                      'enhanced_version': enhanced, 'supported_yapfast_version': args.app_version,
                      'patched_sha256': manifest['patched_sha256']}, indent=2))


if __name__ == '__main__':
    main()
