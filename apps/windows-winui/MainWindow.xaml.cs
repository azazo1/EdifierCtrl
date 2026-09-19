using EdifierCtrl.Native;
using EdifierCtrl.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;

namespace EdifierCtrl;

public sealed partial class MainWindow : Window
{
    private readonly Dictionary<string, FrameworkElement> _pages = new();
    private readonly EventPump _pump = new();

    public MainWindow()
    {
        InitializeComponent();
        Title = "EdifierCtrl";
        TryMica();
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        TryResize(980, 680);
        SessionState.Changed += OnState;
        Closed += (_, _) =>
        {
            SessionState.Changed -= OnState;
            _pump.Stop();
        };
        _pump.Start();
        SessionState.TryAutoJoin();
        OnState();
        Nav.SelectedItem = Nav.MenuItems[0];
        Show("device");
    }

    private void OnState()
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            StatusLine.Text = SessionState.StatusLine();
            HintLine.Text = SessionState.Hint;
        });
    }

    private void OnNav(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag)
        {
            return;
        }
        Show(tag);
    }

    private void Show(string tag)
    {
        if (!_pages.TryGetValue(tag, out var page))
        {
            page = tag switch
            {
                "control" => new ControlPage(),
                "group" => new GroupPage(),
                "debug" => new DebugPage(),
                _ => new DevicePage(),
            };
            _pages[tag] = page;
        }
        ContentFrame.Content = page;
    }

    private void TryMica()
    {
        try
        {
            SystemBackdrop = new MicaBackdrop();
        }
        catch
        {
            // 旧系统没有 Mica 就用默认背景.
        }
    }

    private void TryResize(int width, int height)
    {
        try
        {
            AppWindow?.Resize(new SizeInt32(width, height));
        }
        catch
        {
            // 忽略缩放失败.
        }
    }
}
