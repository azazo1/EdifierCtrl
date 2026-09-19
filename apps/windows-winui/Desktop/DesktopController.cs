using System.Runtime.InteropServices;
using EdifierCtrl.Infrastructure;
using EdifierCtrl.Native;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

namespace EdifierCtrl.Desktop;

internal sealed class DesktopController
{
    private readonly DispatcherQueue _queue;
    private readonly SingleInstanceController _instance = new();
    private MainWindow? _window;
    private WindowGeometryController? _geometry;
    private TrayIcon? _tray;
    private bool _quitting;
    private bool _allowClose;
    private bool _ready;

    public DesktopController(DispatcherQueue queue) { _queue = queue; }

    public async Task StartAsync(string arguments)
    {
        if (!_instance.Acquire())
        {
            try { await _instance.ForwardAsync(arguments); }
            catch (Exception ex) { AppLog.Error("无法唤起已有实例: " + ex.Message, "desktop"); }
            _instance.Dispose();
            AppLog.Flush();
            Application.Current.Exit();
            return;
        }
        AppLog.SetVerbose(AppPreferences.Current.VerboseLogging);
        AppPreferences.Current.Save();
        AppLog.Info($"EdifierCtrl {AppPaths.Version} 启动, 系统 {Environment.OSVersion.VersionString}.", "desktop");
        _window = new MainWindow();
        ApplyTheme();
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(_window);
        _geometry = new WindowGeometryController(_window.AppWindow, handle, _queue);
        _window.AppWindow.Closing += (_, e) =>
        {
            if (_allowClose) return;
            e.Cancel = true;
            if (_quitting) return;
            if (_tray is null) RequestQuit("关闭主窗口");
            else Hide();
        };
        try { _tray = new TrayIcon(handle, command => _queue.TryEnqueue(() => Dispatch(command))); }
        catch (Exception ex) { AppLog.Warn("托盘不可用, 关闭窗口将退出应用: " + ex.Message, "desktop"); }
        _ready = true;
        _instance.Listen(_ => _queue.TryEnqueue(Show));
        Console.CancelKeyPress += OnCancelKeyPress;
        if (!AppPreferences.Current.StartHidden || _tray is null) Show();
        await AppActions.StartAsync(_queue);
    }

    private void Dispatch(string command)
    {
        AppLog.Info("托盘操作: " + command, "desktop");
        if (command == "quit") RequestQuit("托盘退出");
        else if (command == "show") Show();
        else Navigate(command);
    }

    public void Show()
    {
        if (!_ready || _quitting || _window is null) return;
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(_window);
        if (_window.AppWindow.Presenter is OverlappedPresenter { State: OverlappedPresenterState.Minimized } presenter)
            presenter.Restore();
        _geometry?.PrepareToShow();
        _window.AppWindow.Show();
        _window.Activate();
        SetForegroundWindow(handle);
        AppLog.Info("显示主窗口.", "desktop");
    }

    private void Hide()
    {
        if (_quitting) return;
        _geometry?.Persist();
        _window?.AppWindow.Hide();
        AppLog.Info("主窗口已隐藏, 耳机服务继续运行.", "desktop");
    }

    public void Navigate(string key)
    {
        Show();
        _window?.Navigate(key);
    }

    public void ApplyTheme()
    {
        if (_window?.Content is FrameworkElement root)
            root.RequestedTheme = AppPreferences.Current.Theme switch
            {
                "dark" => ElementTheme.Dark,
                "light" => ElementTheme.Light,
                _ => ElementTheme.Default
            };
    }

    public async void RequestQuit(string reason)
    {
        if (_quitting) return;
        _quitting = true;
        var started = System.Diagnostics.Stopwatch.StartNew();
        AppLog.Info("开始退出: " + reason, "desktop");
        try
        {
            _geometry?.Persist();
            await AppActions.StopAsync();
            await _instance.StopAsync();
        }
        catch (Exception ex) { AppLog.Error("退出收尾失败: " + ex.Message, "desktop"); }
        finally
        {
            Console.CancelKeyPress -= OnCancelKeyPress;
            _tray?.Dispose();
            _geometry?.Dispose();
            _instance.Dispose();
            AppLog.Info($"后台服务已停止, 耗时 {started.Elapsed.TotalSeconds:F2} 秒.", "desktop");
            AppLog.Flush();
            _allowClose = true;
            _window?.Close();
            Application.Current.Exit();
        }
    }

    private void OnCancelKeyPress(object? sender, ConsoleCancelEventArgs e)
    {
        e.Cancel = true;
        _queue.TryEnqueue(() => RequestQuit("终端 Ctrl+C"));
    }

    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] private static extern bool SetForegroundWindow(IntPtr window);
}
