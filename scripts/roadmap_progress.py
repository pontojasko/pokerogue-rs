#!/usr/bin/env python3
"""Render a small SVG progress chart from roadmap checkboxes and visual fixtures."""
import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]


def render() -> str:
    roadmap = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")
    states = re.findall(r"^\s*-\s+\[([ xX])\]", roadmap, flags=re.MULTILINE)
    complete = sum(state.lower() == "x" for state in states)
    remaining = len(states) - complete
    percent = complete * 100 / len(states) if states else 0
    visual_cases = len(json.loads((ROOT / "tools/visual/suite.json").read_text())["cases"])
    left, right = 40, 920
    bar_width = right - left
    progress_width = bar_width * percent / 100
    p = f"{percent:.1f}"
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="960" height="280" viewBox="0 0 960 280" role="img" aria-labelledby="title desc">
<title id="title">PokéRogue Rust porting progress</title>
<desc id="desc">{complete} of {len(states)} roadmap checklist items complete, {remaining} remaining; {visual_cases} declared automated visual scenes.</desc>
<defs>
  <linearGradient id="bg" x2="1" y2="1"><stop stop-color="#111827"/><stop offset="1" stop-color="#1e293b"/></linearGradient>
  <linearGradient id="fill" x2="1"><stop stop-color="#34d399"/><stop offset="1" stop-color="#22d3ee"/></linearGradient>
  <filter id="shadow" x="-10%" y="-50%" width="120%" height="200%"><feGaussianBlur stdDeviation="5"/></filter>
</defs>
<rect width="960" height="280" rx="24" fill="url(#bg)"/>
<circle cx="895" cy="42" r="120" fill="#22d3ee" opacity=".06"/>
<text x="40" y="48" fill="#a5f3fc" font-family="Inter,Arial,sans-serif" font-size="14" font-weight="700" letter-spacing="2">POKEROGUE-RS · ROADMAP PROGRESS</text>
<text x="40" y="108" fill="#f8fafc" font-family="Inter,Arial,sans-serif" font-size="46" font-weight="750">{p}%</text>
<text x="174" y="105" fill="#cbd5e1" font-family="Inter,Arial,sans-serif" font-size="20">of roadmap checklist complete</text>
<rect x="{left}" y="137" width="{bar_width}" height="20" rx="10" fill="#334155"/>
<rect x="{left}" y="137" width="{progress_width:.1f}" height="20" rx="10" fill="url(#fill)" filter="url(#shadow)" opacity=".55"/>
<rect x="{left}" y="137" width="{progress_width:.1f}" height="20" rx="10" fill="url(#fill)"/>
<text x="40" y="193" fill="#6ee7b7" font-family="Inter,Arial,sans-serif" font-size="15" font-weight="700">{complete} complete</text>
<text x="880" y="193" text-anchor="end" fill="#cbd5e1" font-family="Inter,Arial,sans-serif" font-size="15">{remaining} remaining · {len(states)} tasks</text>
<rect x="40" y="218" width="880" height="1" fill="#475569"/>
<text x="40" y="250" fill="#e2e8f0" font-family="Inter,Arial,sans-serif" font-size="14" font-weight="600">{visual_cases} declared visual scenes</text>
<text x="920" y="250" text-anchor="end" fill="#94a3b8" font-family="Inter,Arial,sans-serif" font-size="12">Equal-weight checklist; not % of the original game ported or full parity.</text>
</svg>
'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "docs/roadmap-progress.svg")
    parser.add_argument("--check", action="store_true", help="Fail if the checked-in SVG is out of date")
    args = parser.parse_args()
    expected = render()
    path = args.output.resolve()
    if args.check:
        if not path.exists() or path.read_text(encoding="utf-8") != expected:
            print(f"Progress chart is out of date: {path}", file=sys.stderr)
            return 1
        print(f"Progress chart is current: {path}")
        return 0
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(expected, encoding="utf-8")
    print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
