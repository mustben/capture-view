"""Opt-in Windows UI regression using a real card and native window messages.
Exercises the OS modal sizing loop, video-child double-click, F11/Escape, normal close.
Run after copying the release binary to CaptureView.exe in the repository root.
"""
import ctypes as c
from ctypes import wintypes as w
from pathlib import Path
import os
import subprocess
import time


def wait_for(proc, condition, description, timeout=10):
    deadline = time.monotonic()+timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise AssertionError(f'Process exited ({proc.returncode}) while waiting for {description}')
        result = condition()
        if result:
            return result
        time.sleep(.03)  # Poll interval, not proof of completion.
    raise AssertionError(f'Timed out waiting for {description}; pid={proc.pid}')


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
    class GUIThreadInfo(c.Structure):
        _fields_ = [('cbSize', w.DWORD), ('flags', w.DWORD)] + [
            (name, w.HWND) for name in ['active', 'focus', 'capture', 'menu', 'move', 'caret']
        ] + [('rect', w.RECT)]
    u.GetGUIThreadInfo.argtypes = [w.DWORD, c.POINTER(GUIThreadInfo)]
    u.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
    u.EnumWindows.argtypes = [c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM), w.LPARAM]
    callback_type = u.EnumWindows.argtypes[0]
    proc = subprocess.Popen([str(root/'CaptureView.exe')], cwd=root)
    hwnd = None
    try:
        def find(handle, _):
            nonlocal hwnd
            pid = w.DWORD()
            u.GetWindowThreadProcessId(handle, c.byref(pid))
            name = c.create_unicode_buffer(256)
            u.GetClassNameW(handle, name, len(name))
            if pid.value == proc.pid and name.value == 'CaptureViewWindow' and u.GetWindow(handle, 5):
                hwnd = handle
                return False
            return True

        def locate():
            u.EnumWindows(callback_type(find), 0)
            return hwnd
        wait_for(proc, locate, 'CaptureViewWindow with video renderer (not an error dialog)', 20)

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
        thread_id = u.GetWindowThreadProcessId(hwnd, None)
        def sizing():
            info = GUIThreadInfo()
            info.cbSize = c.sizeof(info)
            assert u.GetGUIThreadInfo(thread_id, c.byref(info)), c.WinError(c.get_last_error())
            return bool(info.flags & 2)  # GUI_INMOVESIZE
        wait_for(proc, sizing, 'entry into Windows modal sizing loop')
        for vk in [0x27]*12 + [0x28]*8 + [0x0D]:
            key(vk)
        wait_for(proc, lambda: not sizing(), 'exit from Windows modal sizing loop')
        resized = wait_for(proc, lambda: size(hwnd, True) if size(hwnd, True) != before else None,
                           'native modal resize')
        cw, ch = resized
        ew = min(cw, round(ch*aspect))
        eh = min(ch, round(cw/aspect))
        def renderer_fits():
            rw, rh = size(child)
            return (rw, rh) if abs(rw-ew) <= 2 and abs(rh-eh) <= 2 else None
        rw, rh = wait_for(proc, renderer_fits, f'renderer fit for client {resized}, expected {(ew, eh)}')

        def fullscreen():
            return bool(u.GetWindowLongPtrW(hwnd, -16) & 0x80000000)

        # Explicit hardware-only refresh: exercise the real renderer's keyboard forwarding.
        log_path = Path(os.environ['LOCALAPPDATA'])/'CaptureView/last-run.log'
        def refresh_count():
            try:
                return log_path.read_text(encoding='utf-8').count('Refresh confirmed:')
            except (FileNotFoundError, PermissionError):
                return 0
        def refresh():
            previous = refresh_count()
            child = u.GetWindow(hwnd, 5)
            post(child, 0x100, 0x74)  # F5 on the real video child
            post(child, 0x101, 0x74)
            wait_for(proc, lambda: refresh_count() > previous, 'video/audio graph refresh confirmation', 30)
            self_child = u.GetWindow(hwnd, 5)  # old renderer HWND was destroyed
            assert self_child, 'Refreshed graph has no renderer child'
            return self_child
        child = refresh()
        assert not fullscreen() and size(hwnd, True) == resized, 'Refresh changed window placement'
        rw, rh = size(child)

        # Send the double-click to the actual VIDEO CHILD, not the parent shell.
        post(child, 0x203, 1, ((rh//2)<<16) | (rw//2))
        wait_for(proc, fullscreen, 'video-surface double-click fullscreen')
        key(0x1B)
        wait_for(proc, lambda: not fullscreen() and size(hwnd, True) == resized, 'Escape placement restoration')
        key(0x7A)
        wait_for(proc, fullscreen, 'F11 fullscreen')
        fullscreen_size = size(hwnd, True)
        child = refresh()
        assert fullscreen() and size(hwnd, True) == fullscreen_size, 'Refresh lost fullscreen placement'
        key(0x1B)
        wait_for(proc, lambda: not fullscreen() and size(hwnd, True) == resized, 'final placement restoration')
        post(hwnd, 0x10)
        assert proc.wait(timeout=10) == 0, 'Normal close failed'
        print('PASS: native modal resize and renderer fit; child-video double-click; F11/Escape; exact size restoration; windowed/fullscreen F5 refresh; normal close')
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
