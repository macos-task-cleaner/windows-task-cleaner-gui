using System;
using System.Runtime.InteropServices;
using TaskCleaner.Core.Native;

namespace TaskCleaner.WinUI.Helpers
{
    public static class WindowPositionHelper
    {
        public static void PositionAboveTaskbar(IntPtr hWnd, int windowWidth, int windowHeight)
        {
            var data = new Win32PInvoke.APPBARDATA
            {
                cbSize = (uint)Marshal.SizeOf<Win32PInvoke.APPBARDATA>()
            };

            IntPtr res = Win32PInvoke.SHAppBarMessage(Win32PInvoke.ABM_GETTASKBARPOS, ref data);

            int x, y;
            if (res != IntPtr.Zero)
            {
                // uEdge: 0 = Left, 1 = Top, 2 = Right, 3 = Bottom
                switch (data.uEdge)
                {
                    case 1: // Top
                        x = data.rc.right - windowWidth - 12;
                        y = data.rc.bottom + 12;
                        break;
                    case 0: // Left
                        x = data.rc.right + 12;
                        y = data.rc.bottom - windowHeight - 12;
                        break;
                    case 2: // Right
                        x = data.rc.left - windowWidth - 12;
                        y = data.rc.bottom - windowHeight - 12;
                        break;
                    case 3: // Bottom (Windows 11 默认标准位置)
                    default:
                        x = data.rc.right - windowWidth - 16;
                        y = data.rc.top - windowHeight - 12;
                        break;
                }
            }
            else
            {
                // 默认居右下角安全边距
                x = 1920 - windowWidth - 20;
                y = 1080 - windowHeight - 60;
            }

            // 应用 Windows 11 原生圆角
            int cornerPreference = Win32PInvoke.DWMWCP_ROUND;
            Win32PInvoke.DwmSetWindowAttribute(
                hWnd,
                Win32PInvoke.DWMWA_WINDOW_CORNER_PREFERENCE,
                ref cornerPreference,
                sizeof(int));

            SetWindowPos(hWnd, IntPtr.Zero, x, y, windowWidth, windowHeight, SWP_NOZORDER | SWP_NOACTIVATE);
        }

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

        private const uint SWP_NOZORDER = 0x0004;
        private const uint SWP_NOACTIVATE = 0x0010;
    }
}
