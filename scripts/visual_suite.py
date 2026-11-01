#!/usr/bin/env python3
"""Capture the declared visual cases and gate explicit, locally reviewed baselines."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time
from visual_parity import ROOT, fingerprint


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record-baselines',action='store_true',help='Explicitly snapshot current captures; review reports first. Existing baselines are never overwritten.')
    parser.add_argument('--baseline-dir',type=Path,default=ROOT/'visual-artifacts/baselines')
    parser.add_argument('--output',type=Path,default=ROOT/'visual-artifacts/suite')
    parser.add_argument('--native-backend',choices=['auto','xvfb','display'],default='auto')
    parser.add_argument('--scene', help='Run only one declared scene (also useful for reviewing a new baseline)')
    args=parser.parse_args()
    cases=json.loads((ROOT/'tools/visual/suite.json').read_text())['cases']
    if args.scene:
        cases=[case for case in cases if case['scene']==args.scene]
        if not cases:
            parser.error(f'Undeclared scene: {args.scene}')
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
    baseline_dir=args.baseline_dir.resolve()
    if output == baseline_dir or output in baseline_dir.parents or baseline_dir in output.parents:
        parser.error('Output and baseline directories must not overlap')
    records=[]
    for case in cases:
        scene=case['scene'];folder=output/scene;baseline=baseline_dir/scene
        command=[sys.executable,str(ROOT/'scripts/visual_parity.py'),'--scene',scene,'--output',str(folder),'--native-backend',args.native_backend]
        if 'max_parity_ratio' in case:
            command+=['--max-parity-ratio',str(case['max_parity_ratio'])]
        if not args.record_baselines:
            meta_file=baseline/'metadata.json'
            if not meta_file.exists():
                records.append({'scene':scene,'status':'missing-baseline'})
                continue
            meta=json.loads(meta_file.read_text())
            if fingerprint(ROOT/f'tools/visual/scenes/{scene}.json')!=meta['fixture_sha256'] or fingerprint(baseline/'rust.png')!=meta['rust_sha256']:
                records.append({'scene':scene,'status':'baseline-provenance-mismatch'})
                continue
            command+=['--baseline',str(baseline/'rust.png')]
        elif baseline.exists():
            records.append({'scene':scene,'status':'refused-to-overwrite-baseline'})
            continue
        started=time.monotonic()
        completed=subprocess.run(command,cwd=ROOT,timeout=240)
        report=json.loads((folder/'report.json').read_text())
        record={'scene':scene,'status':report['status'],'seconds':round(time.monotonic()-started,2),'parity':report.get('parity'),'regression':report.get('regression')}
        if completed.returncode==0 and args.record_baselines:
            baseline.mkdir(parents=True)
            shutil.copy2(folder/'rust.png',baseline/'rust.png')
            shutil.copy2(folder/'report.json',baseline/'review-report.json')
            meta={'fixture_sha256':report['fixture_sha256'],'rust_sha256':fingerprint(baseline/'rust.png'),'source_pins':report['source_pins'],'browser':report['reference']['browser'],'native_backend':report['native_backend']}
            (baseline/'metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
            record['status']='baseline-recorded'
        records.append(record)
        # Stop on actual capture/parity/regression failure, preserving its diagnostics.
        if completed.returncode:
            break
    success=len(records)==len(cases) and all(r['status'] in ['baseline-recorded','compared'] for r in records)
    summary={'status':'passed' if success else 'failed','full_visual_parity':False,'cases':records,'note':'Regression gate validates captured Rust images. Original parity is bounded by each harness scope.'}
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    import html
    page='<!doctype html><meta charset="utf-8"><title>Visual suite</title><h1>Visual suite</h1><pre>'+html.escape(json.dumps(summary,indent=2))+'</pre>'
    for record in records:
        scene=record['scene']
        if (output/scene/'report.html').exists():
            page+=f'<p><a href="{scene}/report.html">{scene}</a></p>'
    (output/'index.html').write_text(page)
    print(output/'index.html')
    return 0 if success else 1


if __name__=='__main__':
    sys.exit(main())
