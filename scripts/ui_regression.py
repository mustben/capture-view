"""Opt-in Windows UI regression using a real card and native window messages.
Exercises the OS modal sizing loop, video-child double-click, F11/Escape, normal close.
Run after copying the release binary to CaptureView.exe in the repository root.
"""
import ctypes as c
from ctypes import wintypes as w
from pathlib import Path
import subprocess
import time


def run():
    root = Path(__file__).resolve().parents[1]
    u = c.WinDLL('user32', use_last_error=True)
    u.SetProcessDpiAwarenessContext.argtypes = [w.HANDLE]
    u.SetProcessDpiAwarenessContext(w.HANDLE(-4))
    u.GetWindow.argtypes = [w.HWND, w.UINT]
    u.GetWindow.restype = w.HWND
    u.GetWindowRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
    u.GetClientRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
    u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
    u.GetWindowLongPtrW.argtypes = [w.HWND, c.c_int]
    u.GetWindowLongPtrW.restype = c.c_ssize_t
    u.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
    u.EnumWindows.argtypes = [c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM), w.LPARAM]
    callback_type = u.EnumWindows.argtypes[0]
    proc = subprocess.Popen([str(root/'CaptureView.exe')], cwd=root)
    hwnd = None
    try:
        def find(handle, _):
            nonlocal hwnd
            pid = w.DWORD()
            u.GetWindowThreadProcessId(handle, c.byref(pid))
            if pid.value == proc.pid and u.GetWindow(handle, 5):
                hwnd = handle
                return False
            return True

        deadline = time.monotonic()+15
        while hwnd is None and time.monotonic() < deadline:
            u.EnumWindows(callback_type(find), 0)
            time.sleep(.1)
        assert hwnd is not None, 'Capture window/renderer did not open'

        def size(handle, client=False):
            rect = w.RECT()
            fn = u.GetClientRect if client else u.GetWindowRect
            assert fn(handle, c.byref(rect)), c.WinError(c.get_last_error())
            return rect.right-rect.left, rect.bottom-rect.top

        def post(handle, message, wp=0, lp=0):
            assert u.PostMessageW(handle, message, wp, lp), c.WinError(c.get_last_error())

        def key(vk):
            post(hwnd, 0x100, vk)
            post(hwnd, 0x101, vk)

        child = u.GetWindow(hwnd, 5)
        vw, vh = size(child)
        assert vw > 0 and vh > 0
        aspect = vw/vh
        before = size(hwnd, True)
        # SC_SIZE + WMSZ_BOTTOMRIGHT starts Windows' real nested move/size loop.
        post(hwnd, 0x112, 0xF008)
        time.sleep(.25)
        for vk in [0x27]*12 + [0x28]*8 + [0x0D]:
            key(vk)
            time.sleep(.03)
        time.sleep(.3)
        resized = size(hwnd, True)
        assert resized != before, 'Native modal resize did not change the window'
        cw, ch = resized
        rw, rh = size(child)
        ew = min(cw, round(ch*aspect))
        eh = min(ch, round(cw/aspect))
        assert abs(rw-ew) <= 2 and abs(rh-eh) <= 2, (resized, (rw,rh), (ew,eh))

        def fullscreen():
            return bool(u.GetWindowLongPtrW(hwnd, -16) & 0x80000000)

        # Send the double-click to the actual VIDEO CHILD, not the parent shell.
        post(child, 0x203, 1, ((rh//2)<<16) | (rw//2))
        time.sleep(.3)
        assert fullscreen(), 'Video-surface double-click was not forwarded'
        key(0x1B)
        time.sleep(.3)
        assert not fullscreen() and size(hwnd, True) == resized, 'Escape did not restore placement'
        key(0x7A)
        time.sleep(.3)
        assert fullscreen(), 'F11 did not enter fullscreen'
        key(0x1B)
        time.sleep(.3)
        assert not fullscreen() and size(hwnd, True) == resized
        post(hwnd, 0x10)
        assert proc.wait(timeout=10) == 0, 'Normal close failed'
        print('PASS: native modal resize and renderer fit; child-video double-click; F11/Escape; exact size restoration; normal close')
        print({'before': before, 'resized': resized, 'renderer': (rw,rh)})
    finally:
        if proc.poll() is None:
            if hwnd:
                u.PostMessageW(hwnd, 0x10, 0, 0)
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.terminate()  # Only the test process launched above.
                proc.wait(timeout=5)


if __name__ == '__main__':
    run()
