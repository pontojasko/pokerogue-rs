#!/usr/bin/env python3
"""Render the pixel-art roadmap status card from checklist and visual fixtures."""
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
    cells, cell_width, gap = 55, 14, 2
    filled = round(cells * percent / 100)
    tiles = "\n".join(
        f'<rect x="{40 + i * (cell_width + gap)}" y="150" width="{cell_width}" height="24" fill="{"#65e572" if i < filled else "#29374c"}"/>'
        for i in range(cells)
    )
    p = f"{percent:.1f}%"
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="960" height="320" viewBox="0 0 960 320" role="img" aria-labelledby="title desc" shape-rendering="crispEdges">
<title id="title">PokéRogue Rust porting progress</title>
<desc id="desc">{complete} of {len(states)} roadmap checklist items complete, {remaining} remaining; {visual_cases} declared automated visual scenes.</desc>
<defs>
  <pattern id="stars" width="32" height="32" patternUnits="userSpaceOnUse"><rect x="7" y="9" width="2" height="2" fill="#40516c"/><rect x="23" y="25" width="2" height="2" fill="#40516c"/></pattern>
</defs>
<rect width="960" height="320" fill="#0b1020"/>
<rect x="8" y="8" width="944" height="304" fill="#172338"/>
<rect x="16" y="16" width="928" height="288" fill="url(#stars)"/>
<rect x="24" y="24" width="912" height="272" fill="#111a2b"/>
<rect x="32" y="32" width="896" height="256" fill="#18263c"/>
<text x="48" y="66" fill="#9be7ff" font-family="monospace" font-size="15" font-weight="700" letter-spacing="2">POKEROGUE-RS // PORTING ROADMAP</text>
<text x="48" y="126" fill="#f7f5df" font-family="monospace" font-size="42" font-weight="700">{p}</text>
<text x="260" y="112" fill="#d5deeb" font-family="monospace" font-size="16">CHECKLIST COMPLETE</text>
<text x="260" y="134" fill="#91a4bc" font-family="monospace" font-size="13">MILESTONE TASKS · EQUAL WEIGHT</text>
<g>{tiles}</g>
<text x="48" y="204" fill="#a5f3a4" font-family="monospace" font-size="14" font-weight="700">{complete} DONE</text>
<text x="250" y="204" fill="#ffcf70" font-family="monospace" font-size="14" font-weight="700">{remaining} TO GO</text>
<text x="910" y="204" text-anchor="end" fill="#e3e8ef" font-family="monospace" font-size="14">{len(states)} TOTAL TASKS</text>
<rect x="48" y="224" width="864" height="2" fill="#364660"/>
<text x="48" y="254" fill="#91a4bc" font-family="monospace" font-size="12">{visual_cases} AUTOMATED VISUAL SCENES</text>
<text x="912" y="254" text-anchor="end" fill="#91a4bc" font-family="monospace" font-size="11">CHECKLIST SHARE ≠ GAME PORTING PERCENTAGE</text>
<text x="48" y="278" fill="#657993" font-family="monospace" font-size="10">PIXEL-ART STATUS CARD · SEE ROADMAP.MD FOR SCOPE AND OPEN MILESTONES</text>
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
