using System.Runtime.InteropServices;
using EdifierCtrl.Infrastructure;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Windows.Graphics;

namespace EdifierCtrl.Desktop;

internal sealed class WindowGeometryController : IDisposable
{
    private readonly AppWindow _window;
    private readonly IntPtr _handle;
    private readonly DispatcherQueueTimer _timer;
    private bool _restoring = true;
    private bool _shown;
    private readonly bool _restoreMaximized;

    public WindowGeometryController(AppWindow window, IntPtr handle, DispatcherQueue queue)
    {
        _window = window;
        _handle = handle;
        _timer = queue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(350);
        _timer.IsRepeating = false;
        _timer.Tick += (_, _) => Persist();
        var preferences = AppPreferences.Current;
        var area = DisplayArea.GetFromWindowId(window.Id, DisplayAreaFallback.Primary).WorkArea;
        var scale = Math.Max(1, GetDpiForWindow(handle)) / 96d;
        var width = Math.Clamp((int)(preferences.WindowWidth * scale), Math.Min(900, area.Width), area.Width);
        var height = Math.Clamp((int)(preferences.WindowHeight * scale), Math.Min(650, area.Height), area.Height);
        window.MoveAndResize(new RectInt32(area.X + (area.Width - width) / 2, area.Y + (area.Height - height) / 2, width, height));
        _restoreMaximized = preferences.Maximized;
        _restoring = false;
        window.Changed += OnChanged;
    }

    public void PrepareToShow()
    {
        if (_shown) return;
        _shown = true;
        if (_restoreMaximized && _window.Presenter is OverlappedPresenter presenter) presenter.Maximize();
    }

    private void OnChanged(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (_restoring || (!args.DidSizeChange && !args.DidPresenterChange)) return;
        _timer.Stop();
        _timer.Start();
    }

    public void Persist()
    {
        if (!_shown || _window.Presenter is not OverlappedPresenter presenter || presenter.State == OverlappedPresenterState.Minimized) return;
        var preferences = AppPreferences.Current;
        preferences.Maximized = presenter.State == OverlappedPresenterState.Maximized;
        if (presenter.State == OverlappedPresenterState.Restored)
        {
            var scale = Math.Max(1, GetDpiForWindow(_handle)) / 96d;
            preferences.WindowWidth = _window.Size.Width / scale;
            preferences.WindowHeight = _window.Size.Height / scale;
        }
        preferences.Save();
    }

    public void Dispose()
    {
        _timer.Stop();
        _window.Changed -= OnChanged;
    }

    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr window);
}
