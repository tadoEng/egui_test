#!/usr/bin/env python3
"""Generate the public synthetic tower fixtures (independently authored).

Not derived from any client or real project: grid, sizes and names are
invented round numbers. Deterministic: the same script always writes the
same bytes. Units are kip/ft/F, as in the committed REAL-03 fixtures.

    python3 -I generate_tower.py <output-dir>

writes tower-v1.e2k and tower-v2.e2k. v2 differs from v1 in two
source-backed ways only: the roof slab is inserted at TOP instead of
MIDDLE, and the podium (L1-L2) perimeter beams are deeper.
"""
import sys
from pathlib import Path

STORIES = 12
TYPICAL_HEIGHT = 13
GROUND_HEIGHT = 16
GRID_X = [0, 30, 60, 90, 120]
GRID_Y = [0, 25, 50, 75]
CORE = [(45, 25), (75, 25), (75, 50), (45, 50)]


def story_name(level):
    return f"L{level}"


def generate(version):
    lines = [
        "$ PROGRAM INFORMATION",
        ' PROGRAM "ETABS" VERSION "23.2.0"',
        "$ CONTROLS",
        ' UNITS "Kip" "ft" "F"',
        "$ STORIES - IN SEQUENCE FROM TOP",
    ]
    for level in range(STORIES, 0, -1):
        height = GROUND_HEIGHT if level == 1 else TYPICAL_HEIGHT
        lines.append(f' STORY "{story_name(level)}" HEIGHT {height}')
    lines.append(' STORY "Base" ELEV 0')

    podium_depth = 3 if version == 2 else 2.5
    lines += [
        "$ FRAME SECTIONS",
        ' FRAMESECTION "C30" MATERIAL "C" SHAPE "Concrete Rectangular" D 2.5 B 2.5',
        ' FRAMESECTION "C24" MATERIAL "C" SHAPE "Concrete Rectangular" D 2 B 2',
        ' FRAMESECTION "B24" MATERIAL "C" SHAPE "Concrete Rectangular" D 2 B 1.5',
        f' FRAMESECTION "BP" MATERIAL "C" SHAPE "Concrete Rectangular" D {podium_depth} B 1.5',
        "$ SLAB PROPERTIES",
        ' SHELLPROP "S8" PROPTYPE "Slab" MATERIAL "C" SLABTHICKNESS 0.75',
        ' SHELLPROP "W12" PROPTYPE "Wall" MATERIAL "C" WALLTHICKNESS 1',
        "$ POINT COORDINATES",
    ]
    points = {}
    for x in GRID_X:
        for y in GRID_Y:
            points[(x, y)] = f"G{x}_{y}"
    for x, y in CORE:
        points[(x, y)] = f"K{x}_{y}"
    for (x, y), name in points.items():
        lines.append(f' POINT "{name}" {x} {y}')

    columns = [(f"C{x}_{y}", points[(x, y)]) for x in GRID_X for y in GRID_Y]
    beams = []
    for y in GRID_Y:
        for a, b in zip(GRID_X, GRID_X[1:]):
            perimeter = y in (GRID_Y[0], GRID_Y[-1])
            beams.append((f"BX{a}_{y}", points[(a, y)], points[(b, y)], perimeter))
    for x in GRID_X:
        for a, b in zip(GRID_Y, GRID_Y[1:]):
            perimeter = x in (GRID_X[0], GRID_X[-1])
            beams.append((f"BY{x}_{a}", points[(x, a)], points[(x, b)], perimeter))
    lines.append("$ LINE CONNECTIVITIES")
    for name, point in columns:
        lines.append(f' LINE "{name}" COLUMN "{point}" "{point}" 1')
    for name, i, j, _ in beams:
        lines.append(f' LINE "{name}" BEAM "{i}" "{j}" 0')

    lines.append("$ AREA CONNECTIVITIES")
    corners = [points[(GRID_X[0], GRID_Y[0])], points[(GRID_X[-1], GRID_Y[0])],
               points[(GRID_X[-1], GRID_Y[-1])], points[(GRID_X[0], GRID_Y[-1])]]
    lines.append(' AREA "F" FLOOR 4 ' + " ".join(f'"{p}"' for p in corners) + " 0 0 0 0")
    core = [points[c] for c in CORE]
    walls = []
    for index, (a, b) in enumerate(zip(core, core[1:] + core[:1])):
        name = f"W{index + 1}"
        walls.append(name)
        lines.append(f' AREA "{name}" PANEL 4 "{a}" "{b}" "{b}" "{a}" 0 0 0 0')

    lines.append("$ LINE ASSIGNS")
    for level in range(STORIES, 0, -1):
        story = story_name(level)
        for name, _ in columns:
            section = "C30" if level <= 4 else "C24"
            lines.append(f' LINEASSIGN "{name}" "{story}" SECTION "{section}"')
        for name, _, _, perimeter in beams:
            section = "BP" if perimeter and level <= 2 else "B24"
            lines.append(f' LINEASSIGN "{name}" "{story}" SECTION "{section}"')

    lines.append("$ AREA ASSIGNS")
    for level in range(STORIES, 0, -1):
        story = story_name(level)
        insertion = "TOP" if version == 2 and level == STORIES else "MIDDLE"
        lines.append(
            f' AREAASSIGN "F" "{story}" OPENING "NO" SECTION "S8" CARDINALPOINT "{insertion}"'
        )
        for name in walls:
            lines.append(
                f' AREAASSIGN "{name}" "{story}" OPENING "NO" SECTION "W12" CARDINALPOINT "MIDDLE"'
            )
    lines.append("$ END OF MODEL FILE")
    return "\n".join(lines) + "\n"


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for version in (1, 2):
        (out / f"tower-v{version}.e2k").write_text(generate(version), newline="\n")


if __name__ == "__main__":
    main()
