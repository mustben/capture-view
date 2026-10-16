"""Windows-only shell regression with cancellation before any driver is opened.
Set CAPTUREVIEW_TEST_EXE to the built executable; no capture hardware required.
"""
import ctypes as c
from ctypes import wintypes as w
import os
import subprocess
import sys
import time
import unittest


@unittest.skipUnless(sys.platform == 'win32' and os.environ.get('CAPTUREVIEW_TEST_EXE'),
                     'Requires Windows and explicit CAPTUREVIEW_TEST_EXE')
class Opening(unittest.TestCase):
    def test_responsive_close_during_simulated_open(self):
        u = c.WinDLL('user32', use_last_error=True)
        callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
        u.EnumWindows.argtypes = [callback, w.LPARAM]
        u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
        u.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
        u.IsWindowVisible.argtypes = [w.HWND]
        u.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
        u.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM,
                                         w.UINT, w.UINT, c.POINTER(c.c_size_t)]
        u.SendMessageTimeoutW.restype = c.c_ssize_t
        env = dict(os.environ, CAPTUREVIEW_TEST_OPEN_DELAY_MS='10000')
        proc = subprocess.Popen([os.environ['CAPTUREVIEW_TEST_EXE']], env=env)
        hwnd = None
        try:
            def find(handle, _):
                nonlocal hwnd
                pid = w.DWORD()
                u.GetWindowThreadProcessId(handle, c.byref(pid))
                name = c.create_unicode_buffer(256)
                u.GetClassNameW(handle, name, len(name))
                if pid.value == proc.pid and name.value == 'CaptureViewWindow' and u.IsWindowVisible(handle):
                    hwnd = handle
                    return False
                return True
            deadline = time.monotonic()+5
            while hwnd is None and time.monotonic() < deadline:
                self.assertIsNone(proc.poll(), 'Exited before showing opening shell')
                u.EnumWindows(callback(find), 0)
                time.sleep(.03)
            self.assertIsNotNone(hwnd, 'Opening shell not visible within 5 seconds')
            result = c.c_size_t()
            self.assertTrue(u.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 1000, c.byref(result)),
                            'Opening shell did not process WM_NULL')
            self.assertTrue(u.PostMessageW(hwnd, 0x10, 0, 0))
            self.assertEqual(proc.wait(timeout=5), 0, 'Cancellation before driver open failed')
        finally:
            if proc.poll() is None:
                proc.terminate()  # Only the process created by this test.
                proc.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
