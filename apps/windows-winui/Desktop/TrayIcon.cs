using System.Runtime.InteropServices;
using EdifierCtrl.Infrastructure;

namespace EdifierCtrl.Desktop;

internal sealed class TrayIcon : IDisposable
{
    private const uint CallbackMessage = 0x8001;
    private readonly IntPtr _window;
    private readonly SubclassProc _procedure;
    private readonly Action<string> _command;
    private readonly uint _taskbarCreated;
    private readonly IntPtr _icon;
    private readonly bool _ownsIcon;
    private NotifyIconData _data;

    public TrayIcon(IntPtr window, Action<string> command)
    {
        _window = window;
        _command = command;
        _procedure = WindowMessage;
        _taskbarCreated = RegisterWindowMessage("TaskbarCreated");
        var iconFile = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        _icon = LoadImage(IntPtr.Zero, iconFile, 1, 32, 32, 0x10);
        _ownsIcon = _icon != IntPtr.Zero;
        if (!_ownsIcon) _icon = LoadIcon(IntPtr.Zero, new IntPtr(32512));
        _data = new NotifyIconData
        {
            Size = (uint)Marshal.SizeOf<NotifyIconData>(), Window = window, Id = 1, Flags = 1 | 2 | 4,
            Callback = CallbackMessage, Icon = _icon, Tip = "EdifierCtrl\n耳机控制与跨设备交接", Info = "", InfoTitle = ""
        };
        if (!SetWindowSubclass(window, _procedure, 1, UIntPtr.Zero)) throw new InvalidOperationException("无法注册托盘窗口事件.");
        if (!Shell_NotifyIcon(0, ref _data))
        {
            Dispose();
            throw new InvalidOperationException("无法添加系统托盘图标.");
        }
    }

    private IntPtr WindowMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam, UIntPtr subclassId, UIntPtr reference)
    {
        if (message == _taskbarCreated) Shell_NotifyIcon(0, ref _data);
        if (message == CallbackMessage)
        {
            var eventId = unchecked((uint)lParam.ToInt64()) & 0xffff;
            if (eventId == 0x0202) _command("show");
            if (eventId == 0x0205) ShowMenu();
            return IntPtr.Zero;
        }
        return DefSubclassProc(hwnd, message, wParam, lParam);
    }

    private void ShowMenu()
    {
        var menu = CreatePopupMenu();
        try
        {
            AppendMenu(menu, 2, UIntPtr.Zero, "EdifierCtrl " + AppPaths.Version);
            AppendMenu(menu, 0x800, UIntPtr.Zero, "");
            AppendMenu(menu, 0, (UIntPtr)1, "显示主窗口");
            AppendMenu(menu, 0, (UIntPtr)2, "跨设备交接");
            AppendMenu(menu, 0, (UIntPtr)3, "应用设置");
            AppendMenu(menu, 0x800, UIntPtr.Zero, "");
            AppendMenu(menu, 0, (UIntPtr)4, "退出");
            GetCursorPos(out var point);
            SetForegroundWindow(_window);
            var selected = TrackPopupMenu(menu, 0x100 | 2, point.X, point.Y, 0, _window, IntPtr.Zero);
            PostMessage(_window, 0, IntPtr.Zero, IntPtr.Zero);
            if (selected != 0) _command(selected switch { 2 => "handoff", 3 => "settings", 4 => "quit", _ => "show" });
        }
        finally { DestroyMenu(menu); }
    }

    public void Dispose()
    {
        Shell_NotifyIcon(2, ref _data);
        RemoveWindowSubclass(_window, _procedure, 1);
        if (_ownsIcon) DestroyIcon(_icon);
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct NotifyIconData
    {
        public uint Size; public IntPtr Window; public uint Id; public uint Flags; public uint Callback; public IntPtr Icon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string Tip;
        public uint State; public uint StateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string Info;
        public uint Version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string InfoTitle;
        public uint InfoFlags; public Guid Guid; public IntPtr BalloonIcon;
    }
    [StructLayout(LayoutKind.Sequential)] private struct Point { public int X; public int Y; }
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    private delegate IntPtr SubclassProc(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam, UIntPtr subclassId, UIntPtr reference);
    [DllImport("comctl32.dll")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool SetWindowSubclass(IntPtr hwnd, SubclassProc proc, nuint id, UIntPtr reference);
    [DllImport("comctl32.dll")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool RemoveWindowSubclass(IntPtr hwnd, SubclassProc proc, nuint id);
    [DllImport("comctl32.dll")] private static extern IntPtr DefSubclassProc(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool Shell_NotifyIcon(uint message, ref NotifyIconData data);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern uint RegisterWindowMessage(string message);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr LoadImage(IntPtr instance, string name, uint type, int cx, int cy, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr LoadIcon(IntPtr instance, IntPtr name);
    [DllImport("user32.dll")] private static extern bool DestroyIcon(IntPtr icon);
    [DllImport("user32.dll")] private static extern IntPtr CreatePopupMenu();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern bool AppendMenu(IntPtr menu, uint flags, UIntPtr id, string text);
    [DllImport("user32.dll")] private static extern uint TrackPopupMenu(IntPtr menu, uint flags, int x, int y, int reserved, IntPtr hwnd, IntPtr rect);
    [DllImport("user32.dll")] private static extern bool DestroyMenu(IntPtr menu);
    [DllImport("user32.dll")] private static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
}
