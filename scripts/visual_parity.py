#!/usr/bin/env python3
"""Offline deterministic Phaser/Macroquad captures and explicit pixel/regression reports."""
import argparse
from contextlib import contextmanager
import select
import tempfile
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SCENES = ROOT / 'tools/visual/scenes'


def run(args, **kwargs):
    return subprocess.run([str(x) for x in args], check=True, timeout=kwargs.pop('timeout', 120), **kwargs)


def pixels(path):
    size = run(['magick', 'identify', '-format', '%w %h', path], capture_output=True).stdout.decode().split()
    width, height = map(int, size)
    data = run(['magick', path, '-alpha', 'on', '-depth', '8', 'rgba:-'], capture_output=True).stdout
    if len(data) != width * height * 4:
        raise ValueError('Unexpected RGBA byte count')
    return width, height, data


def metrics(a, b, tolerance=0):
    if a[:2] != b[:2]:
        raise ValueError(f'Image dimensions differ: {a[:2]} != {b[:2]}')
    width, height, left = a
    right = b[2]
    changed = total = squares = maximum = 0
    min_x, min_y, max_x, max_y = width, height, -1, -1
    # Compare all RGBA channels; alpha differences must not silently disappear.
    for pixel in range(width * height):
        delta = [abs(left[4*pixel+c] - right[4*pixel+c]) for c in range(4)]
        peak = max(delta)
        total += sum(delta)
        squares += sum(d*d for d in delta)
        maximum = max(maximum, peak)
        if peak > tolerance:
            changed += 1
            x, y = pixel % width, pixel // width
            min_x, min_y = min(min_x, x), min(min_y, y)
            max_x, max_y = max(max_x, x), max(max_y, y)
    channels = width * height * 4
    return dict(width=width, height=height, changed_pixels=changed,
                changed_ratio=changed/(width*height), mean_absolute_error=total/channels,
                root_mean_square_error=math.sqrt(squares/channels), maximum_channel_error=maximum,
                tolerance=tolerance, changed_bbox=None if not changed else [min_x, min_y, max_x+1, max_y+1])


def compare(original, rust, output, tolerance=0):
    result = metrics(pixels(original), pixels(rust), tolerance)
    run(['magick', original, rust, '+append', output/'side-by-side.png'])
    run(['magick', original, rust, '-compose', 'difference', '-composite', output/'diff.png'])
    return result


def fingerprint(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


@contextmanager
def native_display(env, backend):
    """Private virtual X server when available, with bounded startup and teardown."""
    binary = shutil.which('Xvfb') or str(ROOT/'tools/visual/runtime/usr/bin/Xvfb')
    if backend == 'display' or (backend == 'auto' and not Path(binary).is_file()):
        yield env, 'display'
        return
    if not Path(binary).is_file():
        raise RuntimeError('Xvfb requested but unavailable; install Xvfb or use --native-backend display')
    read_fd, write_fd = os.pipe()
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen([binary, '-displayfd', str(write_fd), '-screen', '0', '960x540x24', '-nolisten', 'tcp', '-noreset'], pass_fds=(write_fd,), stdout=log, stderr=log)
        os.close(write_fd)
        try:
            if not select.select([read_fd], [], [], 10)[0]:
                raise RuntimeError('Xvfb startup timed out')
            number = os.read(read_fd, 64).decode().strip()
            if not number.isdigit():
                log.seek(0)
                raise RuntimeError('Xvfb startup failed: '+log.read().decode(errors='replace')[-1500:])
            isolated = env.copy()
            isolated['DISPLAY'] = ':'+number
            isolated['LIBGL_ALWAYS_SOFTWARE'] = '1'
            yield isolated, 'xvfb'
        finally:
            os.close(read_fd)
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scene', choices=sorted(p.stem for p in SCENES.glob('*.json')), default='battle-town')
    parser.add_argument('--reference-backend', choices=['source-harness', 'full-game'], default='source-harness',
                        help='Use the original isolated class harness or execute the full local Phaser game')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--compare-only', action='store_true', help='Compare existing original.png/rust.png in output')
    parser.add_argument('--baseline', type=Path, help='Explicit local Rust baseline PNG; never created/updated implicitly')
    parser.add_argument('--max-changed-ratio', type=float, default=0.0, help='Allowed regression vs Rust baseline only')
    parser.add_argument('--max-parity-ratio', type=float, help='Optional separate original-vs-Rust parity gate')
    parser.add_argument('--tolerance', type=int, default=0)
    parser.add_argument('--native-backend', choices=['auto','xvfb','display'], default='auto')
    parser.add_argument('--display', help='Display for unattended native capture; defaults to inherited DISPLAY')
    args = parser.parse_args()
    if not 0 <= args.tolerance <= 255 or not 0 <= args.max_changed_ratio <= 1:
        parser.error('Tolerance must be 0..255 and ratios 0..1')
    if args.max_parity_ratio is not None and not 0 <= args.max_parity_ratio <= 1:
        parser.error('Parity ratio must be 0..1')
    output = (args.output or ROOT/'visual-artifacts'/args.scene).resolve()
    if args.baseline and args.baseline.resolve() in {output/'rust.png', output/'original.png', output/'diff.png', output/'side-by-side.png', output/'regression-diff.png'}:
        parser.error('Baseline must be a separate reviewed file, not an output artifact')
    output.mkdir(parents=True, exist_ok=True)
    report = {'scene':args.scene, 'status':'incomplete', 'reference_mode':args.reference_backend,
              'complete_visual_parity':False, 'capture_origin':'external-unverified' if args.compare_only else 'automated', 'limitations':[
                  'Executes original ArenaBase and, in HUD/UI cases, BattleInfo/PlayerBattleInfo/EnemyBattleInfo classes.',
                  'Full original BattleScene/Pokemon/PhaseManager is not executed; hidden HUD collaborators are stubbed.',
                  'Full Rust UI case still lacks original command/move menu reference; field scope excludes UI in both clients.']}
    try:
        if not shutil.which('magick'):
            raise RuntimeError('ImageMagick (magick) is required for PNG decoding and comparison')
        fixture = SCENES/f'{args.scene}.json'
        report['fixture'] = json.loads(fixture.read_text())
        report['fixture_sha256'] = fingerprint(fixture)
        original, rust = output/'original.png', output/'rust.png'
        if not args.compare_only:
            def capture(command, **kwargs):
                with (output/'capture.log').open('a') as log:
                    try:
                        run(command, stdout=log, stderr=subprocess.STDOUT, **kwargs)
                    except subprocess.SubprocessError as error:
                        raise RuntimeError('Capture failed; see capture.log for browser/display/build diagnostics') from error
            (output/'capture.log').write_text('')
            # Remove stale outputs before capture: failure must never reuse old screenshots.
            for name in ['original.png','rust.png','reference.json','side-by-side.png','diff.png','regression-diff.png']:
                (output/name).unlink(missing_ok=True)
            pins = {'original':'7c0881255af5cd51992e513ff93c2f71adbe40e0',
                    'original/assets':'fb8ccb6d0703ffe48cdf80a8102d5ed943c82dba'}
            report['source_pins'] = {}
            for folder, expected in pins.items():
                actual = run(['git','-C',ROOT/folder,'rev-parse','HEAD'],capture_output=True).stdout.decode().strip()
                if actual != expected:
                    raise RuntimeError(f'{folder} pin mismatch: {actual}; expected {expected}')
                report['source_pins'][folder] = actual
            env = os.environ.copy()
            env.pop('POKEROGUE_VISUAL_BATTLE',None)
            env.pop('POKEROGUE_SCREENSHOT_PATH',None)
            env['POKEROGUE_ASSETS_DIR'] = str(ROOT/'original/assets/images/pokemon')
            if args.display:
                env['DISPLAY'] = args.display
            # Users can run the whole command under xvfb-run; no focus/window automation.
            if args.reference_backend == 'full-game':
                ui_state = report['fixture'].get('ui_state')
                if ui_state not in ('command', 'moves'):
                    raise RuntimeError('Full game references currently support only command and moves fixtures')
                full_game_output = output/'full-game'
                capture(['node',ROOT/'tools/visual/capture-original-game.mjs',full_game_output],cwd=ROOT,env=env,timeout=300)
                source_name = 'A-command.png' if ui_state == 'command' else 'B-moves.png'
                shutil.copy2(full_game_output/source_name, original)
                shutil.copy2(full_game_output/'reference.json', output/'reference.json')
                report['limitations'] = [
                    'Reference is captured from the real local Phaser application and its BattleScene/UI handlers.',
                    'The isolated driver skips first-run gender/onboarding and injects a deterministic starter through the original phase API.',
                    'Only the command and moves screens are currently captured from the full game.'
                ]
            else:
                capture(['node',ROOT/'tools/visual/capture.mjs',args.scene,output],cwd=ROOT,env=env)
            capture(['cargo','build','--offline','-p','pokerogue-client'],cwd=ROOT,timeout=300)
            with native_display(env, args.native_backend) as (capture_env, backend):
                report['native_backend'] = backend
                capture([ROOT/'target/debug/pokerogue-client','--visual-fixture',fixture,'--visual-output',rust],cwd=ROOT,env=capture_env,timeout=60)
            for image in (original,rust):
                if pixels(image)[:2] != (report['fixture']['width'],report['fixture']['height']):
                    raise RuntimeError('Capture resolution does not match fixture')
            report['reference'] = json.loads((output/'reference.json').read_text())
        report['parity'] = compare(original,rust,output,args.tolerance)
        report['captures_sha256'] = {p.name:fingerprint(p) for p in (original,rust)}
        failed = args.max_parity_ratio is not None and report['parity']['changed_ratio'] > args.max_parity_ratio
        if args.baseline:
            baseline = args.baseline.resolve()
            report['regression'] = metrics(pixels(baseline),pixels(rust),args.tolerance)
            report['baseline_sha256'] = fingerprint(baseline)
            report['regression']['passed'] = report['regression']['changed_ratio'] <= args.max_changed_ratio
            run(['magick',baseline,rust,'-compose','difference','-composite',output/'regression-diff.png'])
            failed |= not report['regression']['passed']
        report['status'] = 'gate-failed' if failed else 'compared'
        report['regression_checked'] = args.baseline is not None
        return 1 if failed else 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        report['status'] = 'error'
        report['error'] = str(error)
        print(f'Visual capture/comparison failed: {error}',file=sys.stderr)
        return 2
    finally:
        (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        content = '<!doctype html><meta charset="utf-8"><title>Visual comparison</title><style>body{background:#202020;color:white;font:16px sans-serif}img{max-width:100%}pre{white-space:pre-wrap}</style>'
        import html
        content += '<h1>'+html.escape(args.scene)+'</h1><pre>'+html.escape(json.dumps(report,indent=2))+'</pre>'
        if (output/'capture.log').exists():
            content += '<p><a href="capture.log">Capture diagnostics</a></p>'
        for name in ['original.png','rust.png','side-by-side.png','diff.png','regression-diff.png']:
            if (output/name).exists():
                content += f'<h2>{name}</h2><img src="{name}" alt="{name}">'
        (output/'report.html').write_text(content)
        print(output/'report.html')


if __name__ == '__main__':
    sys.exit(main())
