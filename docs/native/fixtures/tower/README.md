# Public synthetic tower fixture

Independently authored for design and demo work (APP-01 design target, APP-01B).
**Not derived from any client, project or real model**: the grid, sizes and names
are invented round numbers chosen by `generate_tower.py`.

| File | sha256 |
|---|---|
| `tower-v1.e2k` | `be31b83b8fd21a39af99edb96e3df35ffbeb181d289007d7db7dc672ead4011b` |
| `tower-v2.e2k` | `679eede2e7a179a81bbde2644eba34bf827d85cf8742cabe1185931d94b6d88e` |

```bash
python3 -I generate_tower.py <out-dir>   # deterministic; check with fixtures.sha256
```

- Kip/ft/°F; 12 stories plus Base; 4 × 3 bays (30 ft × 25 ft); a central core of four
  1 ft walls; rectangular concrete columns (2.5 ft below L5, 2 ft above) and beams;
  one 0.75 ft slab per story.
- `v2` differs from `v1` in exactly two source-backed ways: the L12 slab is inserted
  at `TOP` instead of `MIDDLE`, and the L1–L2 perimeter beams are 3 ft deep instead
  of 2.5 ft. These are **synthetic snapshots**, not EtabExtension commits.
- Built through the real pinned `ext-e2k` / `lab-e2k-mesh` path
  (`native_e2k --check-only` passes). Both versions have 672 source entities and
  0 withheld. Backend fidelity: v1 = 612 Resolved frames
  (`LocalAxisRotationUnavailable`) + 60 Exact shells; v2 = 612 Resolved frames +
  59 Exact shells + 1 Approximate (`ShellInsertionPointIgnored`, the TOP slab).
- No declared openings: a void would withhold its whole host slab until hole
  subtraction exists, so it is left for a dedicated fixture.
