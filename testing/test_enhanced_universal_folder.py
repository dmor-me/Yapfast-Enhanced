"""Exercise the compiled Inno folder picker without running the manager or installing.

Windows + Inno Setup 6 required. Run with --manager <built-manager.exe>.
Test instrumentation is injected only into a temporary copy of the template.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manager', required=True, type=Path)
    args = parser.parse_args()
    source = (ROOT/'packaging/windows/yapfast-enhanced-universal.iss').read_text(encoding='utf-8-sig')
    # The validated folder must also be the folder handed to the real dispatcher.
    assert "AppDirectory := SelectedYapfastDirectory();" in source
    assert "'bootstrap \"' + AppDirectory" in source
    assert "ExpandConstant('{app}')" not in source
    assert 'CurPageID = InstallDirectoryPage.ID' in source
    assert 'CreateAppDir=no' in source and 'Uninstallable=no' in source
    source = source.replace('[Code]', "[Code]\nprocedure TestExit(Code: Integer); external 'ExitProcess@kernel32.dll stdcall';")
    injection = """
  if ExpandConstant('{param:TESTDEFAULT|no}') <> 'yes' then
    InstallDirectoryPage.Values[0] := ExpandConstant('{param:TESTDIRECTORY}');
  if (SelectedYapfastDirectory() = ExpandConstant('{param:TESTDIRECTORY}')) and
     (ValidYapfastDirectory() = (ExpandConstant('{param:TESTVALID}') = 'yes')) then begin
    SaveStringToFile(ExpandConstant('{param:TESTREPORT}'), 'PASS', False);
    TestExit(0);
  end else begin
    SaveStringToFile(ExpandConstant('{param:TESTREPORT}'), 'FAIL', False);
    TestExit(1);
  end;
"""
    marker = '{ Folder picker initialized. }'
    assert source.count(marker) == 1
    source = source.replace(marker, marker + injection)
    source = source.replace('branding/', str(ROOT/'packaging/windows/branding') + '/')
    source = source.replace('OutputBaseFilename=Yapfast-Enhanced-Universal-Installer',
                            'OutputBaseFilename=Folder-Selection-Test')
    compiler = Path(os.environ['LOCALAPPDATA'])/'Programs/Inno Setup 6/ISCC.exe'
    with tempfile.TemporaryDirectory(prefix='enhanced-folder-test-') as directory:
        temp = Path(directory)
        template = temp/'test.iss'
        template.write_text(source, encoding='utf-8-sig')
        subprocess.run([str(compiler), '/Q', '/DEnhancedVersion=0.4.0.0',
                        f'/DManagerExe={args.manager.resolve()}', f'/O{temp}', str(template)], check=True)
        valid = temp/'Custom Yapfast \u00e9 folder'
        valid.mkdir()
        (valid/'yapfast.exe').write_bytes(b'folder selection fixture; never executed')
        missing = temp/'Missing executable'
        missing.mkdir()
        default = Path(os.environ['LOCALAPPDATA'])/'Programs/Yapfast'
        cases = [(str(valid), True, False), (str(missing), False, False),
                 (str(temp), False, False), ('', False, False),
                 (str(default), (default/'yapfast.exe').is_file(), True)]
        results = []
        for index, (folder, expected, use_default) in enumerate(cases):
            report = temp/f'report-{index}.txt'
            subprocess.run([str(temp/'Folder-Selection-Test.exe'), '/VERYSILENT',
                            '/SUPPRESSMSGBOXES', '/SP-', '/NORESTART',
                            f'/TESTDIRECTORY={folder}', f'/TESTDEFAULT={"yes" if use_default else "no"}',
                            f'/TESTVALID={"yes" if expected else "no"}',
                            f'/TESTREPORT={report}'], check=True, timeout=30,
                           creationflags=subprocess.CREATE_NO_WINDOW)
            assert report.read_text(encoding='ascii') == 'PASS', str(folder)
            results.append(dict(folder=Path(folder).name if folder else "(empty)", default=use_default,
                                executable_present=expected, passed=True))
        print(json.dumps(dict(passed=True, installed=False, cases=results)))


if __name__ == '__main__':
    main()
