#!/usr/bin/env python3
"""Screenshots of the real talos-native binary (APP-01A.2), headless.

Linux container only: Xvfb + Mesa lavapipe (software Vulkan), ImageMagick
`import`, and real X11 input through XTest (libXtst). Public synthetic tower
data only. Not Windows evidence: the owner's Windows check stays required.

For each scale: Model docked, a real pick, the same pick with floating
panels, Results, then the window resized to 900 x 700 points (Model,
floating and docked).
Coordinates are logical points of the default 1280 x 800 layout.

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
x11.XDefaultRootWindow.restype = ctypes.c_ulong
x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x11.XQueryTree.argtypes = [
    ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong),
    ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)),
    ctypes.POINTER(ctypes.c_uint)]
x11.XResizeWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_uint, ctypes.c_uint]
x11.XFree.argtypes = [ctypes.c_void_p]
for fn in (xtst.XTestFakeMotionEvent, xtst.XTestFakeButtonEvent):
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


def resize_top_level(handle, width, height):
    """Resizes the one top-level window (there is no window manager)."""
    root, parent = ctypes.c_ulong(), ctypes.c_ulong()
    children, count = ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    x11.XQueryTree(handle, x11.XDefaultRootWindow(handle), ctypes.byref(root),
                   ctypes.byref(parent), ctypes.byref(children), ctypes.byref(count))
    windows = [children[i] for i in range(count.value)]
    if children:
        x11.XFree(children)
    for window in windows:
        x11.XResizeWindow(handle, window, int(width), int(height))
    x11.XFlush(handle)


def shot(name, width, height):
    subprocess.run(
        ["import", "-window", "root", "-crop", f"{width}x{height}+0+0", "+repage",
         os.path.join(OUT, f"{name}.png")],
        check=True, env={**os.environ, "DISPLAY": DISPLAY})


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
        shot(f"model-docked-{tag}", width, height)
        # A column near the middle of the docked canvas: a real pick through
        # the real viewport.
        click(handle, 600 * s, 430 * s)
        time.sleep(3)
        shot(f"model-docked-picked-{tag}", width, height)
        # The view toolbar's Floating panels toggle.
        click(handle, 629 * s, 691 * s)
        time.sleep(3)
        shot(f"model-floating-picked-{tag}", width, height)
        # Results tab (Xvfb has no window manager to give keyboard focus).
        click(handle, 640 * s, 26 * s)
        time.sleep(3)
        shot(f"results-{tag}", width, height)
        # Back to Model (still floating: the mode is kept), then the 900 x 700
        # minimum, floating and docked.
        click(handle, 548 * s, 26 * s)
        time.sleep(2)
        resize_top_level(handle, 900 * s, 700 * s)
        time.sleep(4)
        shot(f"model-900-floating-{tag}", round(900 * s), round(700 * s))
        # The toolbar's Docked panels toggle at 900 x 700, floating layout.
        click(handle, 410 * s, 592 * s)
        time.sleep(3)
        shot(f"model-900-docked-{tag}", round(900 * s), round(700 * s))
    finally:
        app.terminate()
        app.wait(timeout=10)
