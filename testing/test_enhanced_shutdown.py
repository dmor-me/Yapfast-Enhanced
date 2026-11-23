from pathlib import Path
import json,shutil,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parent.parent

def main():
 import argparse,os
 if os.name!='nt':raise SystemExit('This integration test requires Windows')
 parser=argparse.ArgumentParser(description='Test bounded shutdown with disposable processes, never a live user session')
 parser.add_argument('--fixture',type=Path,required=True)
 parser.add_argument('--manager',type=Path,required=True)
 args=parser.parse_args()
 ROOT.joinpath('.local').mkdir(exist_ok=True)
 qa=Path(tempfile.mkdtemp(prefix='shutdown-qa-',dir=ROOT/'.local'))
 fixture=args.fixture.resolve();manager=args.manager.resolve()
 processes=[]
 def launch(folder,name,mode):
  folder.mkdir(parents=True,exist_ok=True);exe=folder/name;shutil.copy2(fixture,exe)
  ready=folder/(name+'.ready');p=subprocess.Popen([str(exe),mode,str(ready)],creationflags=subprocess.CREATE_NO_WINDOW);processes.append(p)
  deadline=time.monotonic()+10
  while not ready.exists():
   assert p.poll() is None
   if time.monotonic()>deadline:raise AssertionError('Fixture did not start')
   time.sleep(.05)
  return p
 report={}
 try:
  for name,mode in [('graceful','graceful'),('unresponsive','hang')]:
   selected=qa/name;other=qa/(name+'-other')
   p=launch(selected,'yapfast.exe',mode);q=launch(other,'yapfast.exe','hang')
   started=time.perf_counter();result=subprocess.run([str(manager),'stop',str(selected)],capture_output=True,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW,timeout=12)
   elapsed=time.perf_counter()-started
   assert result.returncode==0,(result.stdout,result.stderr)
   p.wait(timeout=2);assert q.poll() is None,'Other installation was closed'
   if mode=='graceful': assert 'gracefully' in result.stdout,(result.stdout,elapsed)
   assert elapsed<10,elapsed
   q.terminate();q.wait(timeout=3)
   report[name]={'seconds':round(elapsed,3),'other_installation':'untouched','message':result.stdout.strip()}
  selected=qa/'helper';p=launch(selected,'yapfast.exe','graceful');helper=launch(selected,'yapfast-plugins.exe','hang')
  started=time.perf_counter();result=subprocess.run([str(manager),'stop',str(selected)],capture_output=True,text=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW,timeout=12)
  assert result.returncode==0,(result.stdout,result.stderr);p.wait(timeout=2);helper.wait(timeout=2)
  report['stuck_helper']={'seconds':round(time.perf_counter()-started,3),'app_and_helper':'closed'}
  started=time.perf_counter();subprocess.run([str(manager),'stop',str(selected)],check=True,capture_output=True,creationflags=subprocess.CREATE_NO_WINDOW,timeout=3)
  report['already_closed_seconds']=round(time.perf_counter()-started,3)
  (ROOT/'.local/update-warning-shutdown-qa.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
  print(json.dumps(report,indent=2))
 finally:
  for p in processes:
   if p.poll() is None:p.kill();p.wait(timeout=3)
  assert qa.resolve().parent==(ROOT/'.local').resolve() and qa.name.startswith('shutdown-qa-')
  shutil.rmtree(qa)

if __name__=='__main__':main()
