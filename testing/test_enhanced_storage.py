"""Check the current Windows installer on an isolated app copy.

For a previous/new disk-and-time comparison use test_enhanced_recovery.py.
"""
import argparse
import json
from pathlib import Path
import tempfile
ROOT=Path(__file__).resolve().parent.parent

def main():
    from test_enhanced_recovery import case
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--official-exe',type=Path,required=True)
    parser.add_argument('--iscc',type=Path,required=True)
    parser.add_argument('--release-dir',type=Path,default=ROOT/'.release/enhanced-v0.4.1.0')
    parser.add_argument('--legacy',action='store_true',help='check a previous installer using NTFS-compressed raw recovery copies')
    args=parser.parse_args()
    qa=Path(tempfile.mkdtemp(prefix='enhanced-storage-',dir=ROOT/'.local'))
    archived=not args.legacy
    result=case(qa,args.release_dir,args.official_exe,args.iscc,archived)
    print(json.dumps({'qa':str(qa),**result},indent=2))
if __name__=='__main__':main()
