#!/usr/bin/env python3
"""Inline the vetted Lucide icons into template.html -> target.html."""
import re
from pathlib import Path

here = Path(__file__).parent


def icon(match):
    name, size = match.group(1), match.group(2) or ""
    svg = (here / "assets" / "icons" / f"{name}.svg").read_text()
    svg = re.sub(r"<!--.*?-->", "", svg, flags=re.S).strip()
    svg = re.sub(r'\s(width|height)="24"', "", svg)
    svg = re.sub(r'class="[^"]*"', f'class="i {size.lstrip(":")}" aria-hidden="true"', svg, count=1)
    return " ".join(svg.split())


html = (here / "template.html").read_text()
(here / "target.html").write_text(re.sub(r"\{\{icon:([a-z0-9-]+)(:sm)?\}\}", icon, html))
