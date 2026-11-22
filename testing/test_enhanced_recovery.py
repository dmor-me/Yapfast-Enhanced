"""Windows recovery regression and disk/time comparison on isolated app copies.

Uses separate uninstall AppIds and preserves the real app/registry entry.
Pass the new staged release, previous release, trusted official executable, and ISCC.
"""
import argparse
import ctypes
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time
import uuid
if os.name == "nt":
    import winreg
import zipfile

ROOT = Path(__file__).resolve().parent.parent

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream,'sha256').hexdigest()

def allocated(path):
    lib=ctypes.WinDLL('kernel32',use_last_error=True)
    call=lib.GetCompressedFileSizeW
    call.argtypes=[ctypes.c_wchar_p,ctypes.POINTER(ctypes.c_ulong)]
    call.restype=ctypes.c_ulong
    high=ctypes.c_ulong();ctypes.set_last_error(0)
    low=call(str(path),ctypes.byref(high))
    if low==0xffffffff and ctypes.get_last_error():raise ctypes.WinError(ctypes.get_last_error())
    return high.value*2**32+low

def folder_bytes(path):
    return sum(allocated(file) for file in path.rglob('*') if file.is_file())

def zip_digest(path,name):
    with zipfile.ZipFile(path) as archive:
        with archive.open(name) as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def run(command,**kwargs):
    return subprocess.run(list(map(str,command)),check=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180,**kwargs)

def case(qa, release, official, iscc, archives, upgrade_from=None):
    name='archives' if archives else 'previous-ntfs'
    work=qa/name;work.mkdir()
    app=work/'app';app.mkdir();shutil.copy2(official,app/'yapfast.exe')
    (app/'account-marker.txt').write_text('keep account and plugin data',encoding='utf-8')
    if upgrade_from is not None:
        # Seed the prior manager's real state/raw recovery layout, then upgrade
        # it using the current installer, with the same account marker intact.
        old=upgrade_from/'bundle'
        previous=app/'.yapfast-enhanced'
        previous.mkdir()
        shutil.copy2(old/'manager.exe',previous/'manager.exe')
        shutil.copytree(old/'payload',previous/'payload')
        run([previous/'manager.exe','install',app,previous/'payload'],stdout=subprocess.DEVNULL)
    bundle=release/'bundle'
    manifest=json.loads((bundle/'payload/manifest.json').read_text(encoding='utf-8'))
    product=re.search(r'^Enhanced version: ([0-9.]+)$', (bundle/'SOURCE.txt').read_text(encoding='utf-8'), re.MULTILINE)
    assert product, 'Bundle must identify its Enhanced version'
    assert digest(official)==manifest['upstream_sha256']
    identity=str(uuid.uuid4()).upper()
    source=(ROOT/'packaging/windows/yapfast-enhanced.iss').read_text(encoding='utf-8')
    source=source.replace('437563B0-0C9A-4B3B-978E-8D057910AB77',identity)
    source=source.replace('branding/',(ROOT/'packaging/windows/branding').as_posix()+'/')
    source=source.replace('branding\\',str(ROOT/'packaging/windows/branding')+'\\')
    if not archives:
        # Reproduce the previously shipped layout using the previous binaries.
        source=source.replace('Flags: unsetntfscompression uninsneveruninstall','Flags: setntfscompression uninsneveruninstall')
        a=source.index('Source: "{#BundleDir}\\payload\\manifest.json"')
        b=source.index('Source: "{#BundleDir}\\LICENSE.txt"',a)
        source=source[:a]+''.join(
            f'Source: "{{#BundleDir}}\\payload\\{file}"; DestDir: "{{app}}\\.yapfast-enhanced\\payload"; Flags: ignoreversion'+
            (' setntfscompression' if file.endswith('.exe') else '')+'\n'
            for file in ['manifest.json','yapfast.exe','yapfast-plugins.exe'])+source[b:]
        source=source.replace("ExpandConstant('{app}\\.yapfast-enhanced\\payload.next.zip')","ExpandConstant('{app}\\.yapfast-enhanced\\payload')")
    template=work/'setup.iss';template.write_text(source,encoding='utf-8')
    run([iscc,'/Q','/DStylePreview','/DAppVersion=0.4.1',f'/DEnhancedVersion={product.group(1)}',f'/DBundleDir={bundle}',f'/O{work}',template])
    started=time.perf_counter()
    run([work/'Yapfast-Enhanced-Installer.exe','/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-','/NOICONS',f'/DIR={app}',f'/LOG={work/"install.log"}'])
    install_seconds=time.perf_counter()-started
    meta=app/'.yapfast-enhanced';manager=meta/'manager.exe'
    assert digest(app/'yapfast.exe')==manifest['patched_sha256']
    assert digest(app/'yapfast-plugins.exe')==manifest['helper_sha256']
    for file in ['yapfast.exe','yapfast-plugins.exe']:
        assert not os.stat(app/file).st_file_attributes&0x800, 'Live files must remain uncompressed'
    if archives:
        backup=meta/'backups'/(manifest['upstream_sha256']+'.exe.zip')
        assert zip_digest(backup,'executable')==manifest['upstream_sha256']
        assert zip_digest(meta/'payload.zip','yapfast.exe')==manifest['patched_sha256']
        assert zip_digest(meta/'payload.zip','yapfast-plugins.exe')==manifest['helper_sha256']
        assert not (meta/'payload').exists()
        assert not list((meta/'backups').glob('*.exe'))
        cold=allocated(backup)+allocated(meta/'payload.zip')
    else:
        backup=meta/'backups'/(manifest['upstream_sha256']+'.exe')
        assert digest(backup)==manifest['upstream_sha256']
        cold=allocated(backup)+folder_bytes(meta/'payload')
    physical=folder_bytes(app)
    key='Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{'+identity+'}_is1'
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER,key,0,winreg.KEY_READ|winreg.KEY_WOW64_64KEY) as registry:
        estimated,_=winreg.QueryValueEx(registry,'EstimatedSize')
        displayed,_=winreg.QueryValueEx(registry,'DisplayVersion')
        assert displayed==product.group(1), (displayed,product.group(1))
    assert abs(estimated*1024-physical)<65536,(estimated,physical)
    timings=[]
    for _ in range(3):
        start=time.perf_counter();run([manager,'repair',app],stdout=subprocess.DEVNULL);timings.append(time.perf_counter()-start)
        assert digest(app/'yapfast.exe')==manifest['patched_sha256']
    # A legitimate official reinstall must also remain repairable offline.
    shutil.copy2(official,app/'yapfast.exe')
    run([manager,'repair',app],stdout=subprocess.DEVNULL)
    assert digest(app/'yapfast.exe')==manifest['patched_sha256']
    start=time.perf_counter();run([manager,'restore',app],stdout=subprocess.DEVNULL);restore_seconds=time.perf_counter()-start
    assert digest(app/'yapfast.exe')==manifest['upstream_sha256']
    assert not (app/'yapfast-plugins.exe').exists()
    assert (app/'account-marker.txt').read_text(encoding='utf-8')=='keep account and plugin data'
    run([meta/'uninstall/unins000.exe','/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART'])
    return {'installed_allocated_bytes':physical,'cold_recovery_allocated_bytes':cold,
            'install_seconds':round(install_seconds,3),'repair_median_seconds':round(statistics.median(timings),3),
            'restore_seconds':round(restore_seconds,3),'windows_estimated_kib':estimated,
            'offline_repair':'passed','official_reinstall_repair':'passed','exact_restore':'passed','account_marker':'preserved'}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release-dir',type=Path,required=True)
    parser.add_argument('--previous-release-dir',type=Path,required=True)
    parser.add_argument('--official-exe',type=Path,required=True)
    parser.add_argument('--iscc',type=Path,required=True)
    args=parser.parse_args()
    qa=Path(tempfile.mkdtemp(prefix='enhanced-archive-qa-',dir=ROOT/'.local'))
    before=case(qa,args.previous_release_dir,args.official_exe,args.iscc,False)
    after=case(qa,args.release_dir,args.official_exe,args.iscc,True)
    assert after['cold_recovery_allocated_bytes']<before['cold_recovery_allocated_bytes']
    result={'qa':str(qa),'previous_ntfs':before,'archives':after,
            'saved_allocated_bytes':before['installed_allocated_bytes']-after['installed_allocated_bytes'],
            'note':'Elapsed times use fresh manager processes with a warm filesystem cache, not a reboot-cold or chat-runtime benchmark.'}
    print(json.dumps(result,indent=2))

if __name__=='__main__':main()
