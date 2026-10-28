#!/usr/bin/env python3
"""Screenshots of the real talos-native binary (APP-01B), headless.

Linux container only: Xvfb + Mesa lavapipe (software Vulkan), ImageMagick
`import`, and real X11 input through XTest (libXtst). Public synthetic tower
data only. Not Windows evidence: the owner's Windows check stays required.

    python3 -I capture_talos.py <talos-native binary> <out-dir>
"""
import ctypes
import os
import subprocess
import sys
import time

BINARY, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
DISPLAY = ":99"

x11 = ctypes.CDLL("libX11.so.6")
xtst = ctypes.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
for fn in (xtst.XTestFakeMotionEvent, xtst.XTestFakeButtonEvent, xtst.XTestFakeKeyEvent):
    fn.argtypes = None
x11.XFlush.argtypes = [ctypes.c_void_p]


def display():
    handle = x11.XOpenDisplay(DISPLAY.encode())
    if not handle:
        raise SystemExit("cannot open X display")
    return handle


def click(handle, x, y):
    xtst.XTestFakeMotionEvent(ctypes.c_void_p(handle), -1, int(x), int(y), 0)
    x11.XFlush(handle)
    time.sleep(0.3)
    for pressed in (1, 0):
        xtst.XTestFakeButtonEvent(ctypes.c_void_p(handle), 1, pressed, 0)
        x11.XFlush(handle)
        time.sleep(0.15)


def chord(handle, *keysyms):
    codes = [x11.XKeysymToKeycode(ctypes.c_void_p(handle), k) for k in keysyms]
    for code in codes:
        xtst.XTestFakeKeyEvent(ctypes.c_void_p(handle), code, 1, 0)
    for code in reversed(codes):
        xtst.XTestFakeKeyEvent(ctypes.c_void_p(handle), code, 0, 0)
    x11.XFlush(handle)


def shot(name, width, height):
    subprocess.run(
        ["import", "-window", "root", "-crop", f"{width}x{height}+0+0", "+repage",
         os.path.join(OUT, f"{name}.png")],
        check=True, env={**os.environ, "DISPLAY": DISPLAY})


XK_CONTROL_L, XK_2 = 0xFFE3, 0x32
if subprocess.run(["pgrep", "-x", "Xvfb"], capture_output=True).returncode != 0:
    subprocess.Popen(["Xvfb", DISPLAY, "-screen", "0", "2100x1400x24"],
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(2)
handle = display()
for scale in ("1", "1.25", "1.5"):
    env = {**os.environ, "DISPLAY": DISPLAY,
           "VK_ICD_FILENAMES": "/usr/share/vulkan/icd.d/lvp_icd.json",
           "WINIT_X11_SCALE_FACTOR": scale}
    app = subprocess.Popen([BINARY], env=env, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)
    try:
        time.sleep(20)
        s = float(scale)
        width, height = round(1280 * s), round(800 * s)
        tag = f"{scale.replace('.', '_')}x"
        shot(f"talos-model-{tag}", width, height)
        # The middle of the model canvas, between the default explorer and
        # inspector panels: a real pick through the real viewport.
        click(handle, 600 * s, 430 * s)
        time.sleep(3)
        shot(f"talos-model-picked-{tag}", width, height)
        chord(handle, XK_CONTROL_L, XK_2)
        time.sleep(3)
        shot(f"talos-results-{tag}", width, height)
    finally:
        app.terminate()
        app.wait(timeout=10)
