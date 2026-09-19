using EdifierCtrl.Desktop;
using EdifierCtrl.Infrastructure;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class PreferencesPage : Page
{
    private bool _initialized;
    private bool _subscribed;
    private bool _syncing;

    public PreferencesPage()
    {
        InitializeComponent();
        _initialized = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (!_subscribed)
        {
            AppPreferences.Changed += OnPreferences;
            SessionState.Changed += OnState;
            _subscribed = true;
        }
        OnPreferences();
        OnState();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        AppPreferences.Changed -= OnPreferences;
        SessionState.Changed -= OnState;
        _subscribed = false;
    }

    private void OnPreferences()
    {
        if (!_initialized)
        {
            return;
        }
        _syncing = true;
        try
        {
            var preferences = AppPreferences.Current;
            StartHiddenSwitch.IsOn = preferences.StartHidden;
            AutoJoinSwitch.IsOn = preferences.AutoJoinGroup;
            AutoJoinSwitch.IsEnabled = preferences.RememberGroup && !string.IsNullOrWhiteSpace(preferences.GroupName);
            AutoJoinDetail.Text = AutoJoinSwitch.IsEnabled ? "使用已记住的组名自动发现其他设备" : "需要先在交接页记住组名";
            LoggingSwitch.IsOn = preferences.VerboseLogging;
            ThemeChoice.SelectedIndex = preferences.Theme switch { "light" => 1, "dark" => 2, _ => 0 };
            DataDirectoryText.Text = "数据目录: " + AppPaths.DataDirectory;
            LogFileText.Text = "日志文件: " + AppPaths.LogFile;
            SaveError.IsOpen = !string.IsNullOrEmpty(preferences.SaveError);
            SaveError.Message = preferences.SaveError ?? "";
        }
        finally
        {
            _syncing = false;
        }
    }

    private void OnState() => CoreVersionText.Text = string.IsNullOrWhiteSpace(SessionState.CoreVersion)
        ? "协议核心尚未加载" : "协议核心 " + SessionState.CoreVersion;

    private void OnStartHidden(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing)
        {
            return;
        }
        AppPreferences.Current.StartHidden = StartHiddenSwitch.IsOn;
        AppPreferences.Current.Save();
    }

    private void OnAutoJoin(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing)
        {
            return;
        }
        AppPreferences.Current.AutoJoinGroup = AutoJoinSwitch.IsOn && AutoJoinSwitch.IsEnabled;
        AppPreferences.Current.Save();
    }

    private void OnLogging(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing)
        {
            return;
        }
        AppPreferences.Current.VerboseLogging = LoggingSwitch.IsOn;
        AppPreferences.Current.Save();
    }

    private void OnTheme(object sender, SelectionChangedEventArgs e)
    {
        if (!_initialized || _syncing || ThemeChoice.SelectedItem is not ComboBoxItem { Tag: string theme })
        {
            return;
        }
        AppPreferences.Current.Theme = theme;
        AppPreferences.Current.Save();
        DesktopCommands.ApplyTheme();
    }

    private void OnHandoff(object sender, RoutedEventArgs e) => DesktopCommands.Navigate("handoff");
    private void OnActivity(object sender, RoutedEventArgs e) => DesktopCommands.Navigate("activity");
    private void OnOpenData(object sender, RoutedEventArgs e) => DesktopCommands.OpenDataDirectory();
    private void OnBluetooth(object sender, RoutedEventArgs e) => DesktopCommands.OpenBluetoothSettings();
    private void OnQuit(object sender, RoutedEventArgs e) => DesktopCommands.Quit();
}
