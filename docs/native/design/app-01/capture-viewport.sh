#!/usr/bin/env bash
# Capture the REAL native wgpu viewport (examples/native_e2k) headlessly.
# Linux container only: Xvfb + Mesa lavapipe (software Vulkan). Owner-approved
# for this container on 2026-10-28; nothing here changes the repo toolchain.
#   apt-get install -y --no-install-recommends mesa-vulkan-drivers libxkbcommon-x11-0
# Usage (from the repo root, after building the example):
#   cargo build --locked --offline --example native_e2k --features real-geometry --target-dir .codex-target
#   docs/native/design/app-01/capture-viewport.sh docs/native/fixtures/tower/tower-v2.e2k out.png
set -euo pipefail
fixture=$1 out=$2
export DISPLAY=:99 VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json WINIT_X11_SCALE_FACTOR=1.75
pgrep -x Xvfb >/dev/null || { Xvfb :99 -screen 0 2100x1400x24 >/dev/null 2>&1 & sleep 2; }
./.codex-target/debug/examples/native_e2k "$fixture" >/dev/null 2>&1 &
app=$!
sleep 20
import -window root "$out.full.png"
kill "$app"; wait "$app" 2>/dev/null || true
# Keep only the viewport interior: drops the example's header and its two
# diagnostic text overlays; the 3D pixels are unmodified.
convert "$out.full.png" -crop 1880x975+20+278 +repage "$out"
rm "$out.full.png"
