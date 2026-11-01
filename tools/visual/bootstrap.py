#!/usr/bin/env python3
"""Prepare local visual dependencies without changing system/browser configuration."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile
import urllib.request

ROOT=Path(__file__).resolve().parents[2]
ASSET_PIN='fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba'
# These bytes stay only in the original local checkout (never committed/bundled).
HUD_FILES=['fonts/pokemon-emerald-pro.ttf','images/ui/overlay_hp.json','images/ui/overlay_hp_boss.json','images/ui/numbers.json','images/ui/numbers.png','images/ui/overlay_exp.png']+[f'images/ui/text_images/en/battle_ui/{name}.png' for name in ['overlay_lv','overlay_hp_label','overlay_exp_label']]
XVFB_URL='https://stable-mirror.omarchy.org/extra/os/x86_64/xorg-server-xvfb-21.1.25-2-x86_64.pkg.tar.zst'
XVFB_SHA256='52c6d0394109b0bdfd0e4d00dd8d89d7e38bfe7576d96145179f216480eb7564'

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--xvfb',action='store_true',help='Download a pinned x86_64 Linux Xvfb binary to ignored runtime/; needs local Xorg libraries and bsdtar')
    args=parser.parse_args()
    actual=subprocess.check_output(['git','-C',str(ROOT/'original/assets'),'rev-parse','HEAD'],text=True).strip()
    if actual!=ASSET_PIN:
        raise SystemExit('Asset checkout pin mismatch; not modifying it')
    for name in HUD_FILES:
        target=ROOT/'original/assets'/name
        if target.exists():
            continue
        content=subprocess.check_output(['git','-C',str(ROOT/'original/assets'),'show',f'{ASSET_PIN}:{name}'])
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(content)
    subprocess.run(['npm','ci','--prefix',str(ROOT/'tools/visual'),'--offline','--ignore-scripts'],check=True)
    if args.xvfb:
        runtime=ROOT/'tools/visual/runtime'
        if not (runtime/'usr/bin/Xvfb').exists():
            with tempfile.TemporaryDirectory() as directory:
                archive=Path(directory)/'xvfb.pkg.tar.zst'
                with urllib.request.urlopen(XVFB_URL,timeout=30) as response:
                    data=response.read(10*1024*1024)
                if hashlib.sha256(data).hexdigest()!=XVFB_SHA256:
                    raise SystemExit('Xvfb package checksum mismatch')
                archive.write_bytes(data)
                runtime.mkdir(parents=True,exist_ok=True)
                subprocess.run(['bsdtar','-xf',str(archive),'-C',str(runtime),'usr/bin/Xvfb'],check=True)
    print('Local visual tools ready. Run python3 scripts/visual_suite.py')

if __name__=='__main__':
    main()
