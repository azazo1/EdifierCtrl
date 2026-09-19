using EdifierCtrl.Desktop;
using EdifierCtrl.Infrastructure;
using EdifierCtrl.Native;
using EdifierCtrl.Pages;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace EdifierCtrl;

public sealed partial class MainWindow : Window
{
    private readonly Dictionary<string, FrameworkElement> _pages = new();
    private bool _selecting;
    private bool _aboutOpen;

    public MainWindow()
    {
        InitializeComponent();
        Title = "EdifierCtrl";
        try { SystemBackdrop = new MicaBackdrop(); }
        catch (Exception ex) { AppLog.Debug("当前系统未启用 Mica: " + ex.Message, "desktop"); }
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.ButtonBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        Root.ActualThemeChanged += (_, _) => UpdateCaptionColors();
        UpdateCaptionColors();
        var icon = Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico");
        if (File.Exists(icon)) AppWindow.SetIcon(icon);
        SessionState.Changed += OnState;
        Closed += (_, _) => SessionState.Changed -= OnState;
        VersionButton.Content = AppPaths.Version + "  ·  关于";
        Navigate("headphones");
        OnState();
    }

    private void UpdateCaptionColors()
    {
        var dark = Root.ActualTheme == ElementTheme.Dark;
        AppWindow.TitleBar.ButtonForegroundColor = dark ? Colors.White : Colors.Black;
        AppWindow.TitleBar.ButtonInactiveForegroundColor = dark ? Colors.Gray : Colors.DarkGray;
    }

    private void OnState()
    {
        ConnectionStatus.Text = SessionState.Operation ?? (SessionState.Connected ? "控制已连接" : "未连接耳机");
        BusyRing.IsActive = SessionState.Busy;
        BusyRing.Visibility = SessionState.Busy ? Visibility.Visible : Visibility.Collapsed;
        ServiceLabel.Text = SessionState.Ready ? "后台服务运行中" : "耳机服务未就绪";
        Notice.Title = SessionState.NoticeTitle ?? "";
        Notice.Message = SessionState.NoticeDetail ?? "";
        Notice.Severity = SessionState.NoticeIsError ? InfoBarSeverity.Error : InfoBarSeverity.Informational;
        Notice.IsOpen = !string.IsNullOrEmpty(SessionState.NoticeTitle);
    }

    private void OnNoticeClosed(InfoBar sender, InfoBarClosedEventArgs args) => SessionState.ClearNotice();

    private void OnNavigation(object sender, SelectionChangedEventArgs args)
    {
        if (!_selecting && args.AddedItems.FirstOrDefault() is ListViewItem { Tag: string key }) Navigate(key);
    }

    public void Navigate(string key)
    {
        if (key is not ("headphones" or "devices" or "handoff" or "settings" or "activity")) key = "headphones";
        _selecting = true;
        Navigation.SelectedItem = Navigation.Items.OfType<ListViewItem>().FirstOrDefault(item => Equals(item.Tag, key));
        FooterNavigation.SelectedItem = FooterNavigation.Items.OfType<ListViewItem>().FirstOrDefault(item => Equals(item.Tag, key));
        _selecting = false;
        var heading = key switch
        {
            "devices" => ("设备连接", "选择你的耳机, 让音乐准备就绪."),
            "handoff" => ("跨设备交接", "让声音在你的设备之间自然流转."),
            "settings" => ("应用设置", "按你的习惯, 安静地在后台工作."),
            "activity" => ("活动记录", "了解每一次连接与交接发生了什么."),
            _ => ("我的耳机", "此刻的声音, 由你掌控.")
        };
        PageTitle.Text = heading.Item1;
        PageSubtitle.Text = heading.Item2;
        if (!_pages.TryGetValue(key, out var page))
        {
            page = key switch
            {
                "devices" => new DevicePage(), "handoff" => new GroupPage(),
                "settings" => new PreferencesPage(), "activity" => new DebugPage(),
                _ => new ControlPage()
            };
            _pages[key] = page;
        }
        ContentFrame.Content = page;
        AppLog.Debug("切换页面: " + key, "ui");
    }

    private async void OnVersion(object sender, RoutedEventArgs e)
    {
        if (_aboutOpen) return;
        _aboutOpen = true;
        try
        {
            var panel = new StackPanel { Spacing = 12 };
            panel.Children.Add(new TextBlock { Text = "声音, 随你而行.", FontSize = 18 });
            panel.Children.Add(new TextBlock { Text = "应用版本: " + AppPaths.Version });
            panel.Children.Add(new TextBlock { Text = "协议核心: " + (string.IsNullOrEmpty(SessionState.CoreVersion) ? "尚未加载" : SessionState.CoreVersion) });
            var releases = new HyperlinkButton { Content = "查看项目与发行版本", NavigateUri = new Uri("https://github.com/azazo1/EdifierCtrl/releases") };
            panel.Children.Add(releases);
            await new ContentDialog { Title = "EdifierCtrl", Content = panel, CloseButtonText = "关闭", XamlRoot = Root.XamlRoot }.ShowAsync();
        }
        finally { _aboutOpen = false; }
    }
}
