#!/usr/bin/env python3
"""Package the online dispatcher separately from version-specific offline installers."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manager', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('enhanced_package', ROOT/'scripts/package-enhanced-inplace.py')
    package = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(package)
    version = package.enhanced_version()
    package.executable(args.manager)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    compiler = Path(os.environ['LOCALAPPDATA'])/'Programs/Inno Setup 6/ISCC.exe'
    subprocess.run([str(compiler), f'/DEnhancedVersion={version}',
                    f'/DManagerExe={args.manager.resolve()}', f'/O{output}',
                    str(ROOT/'packaging/windows/yapfast-enhanced-universal.iss')], check=True)
    installer = output/'Yapfast-Enhanced-Universal-Installer.exe'
    checksums = output/'SHA256SUMS.txt'
    existing = checksums.read_text(encoding='ascii') if checksums.exists() else ''
    lines = [line for line in existing.splitlines() if not line.endswith('  '+installer.name)]
    lines.append(f'{package.digest(installer)}  {installer.name}')
    checksums.write_text('\n'.join(lines)+'\n', encoding='ascii')
    print(json.dumps(dict(installer=str(installer), version=version,
                          bytes=installer.stat().st_size, sha256=package.digest(installer))))


if __name__ == '__main__':
    main()
