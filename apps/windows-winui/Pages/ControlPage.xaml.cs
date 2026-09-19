using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class ControlPage : Page
{
    private bool _syncing;

    public ControlPage()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            SessionState.Changed += OnState;
            OnState();
        };
        Unloaded += (_, _) => SessionState.Changed -= OnState;
    }

    private void OnState()
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            _syncing = true;
            Lead.Text = SessionState.Connected
                ? "改设置会发到已连接的耳机."
                : "先到设备页连接耳机.";
            InfoBattery.Text = "电量 " + (SessionState.Battery is int b ? b + "%" : "-");
            InfoMac.Text = "MAC " + (SessionState.Mac ?? "-");
            InfoFirmware.Text = "固件 " + (SessionState.Firmware ?? "-");
            if (string.IsNullOrEmpty(NameBox.Text) && !string.IsNullOrEmpty(SessionState.DeviceName))
            {
                NameBox.Text = SessionState.DeviceName;
            }
            Ambient.Value = SessionState.AmbientVolume;
            Prompt.Value = SessionState.PromptVolume;
            ShutdownMinutes.Value = SessionState.ShutdownMinutes;
            ShutdownSwitch.IsOn = SessionState.ShutdownOn;
            GameMode.IsOn = SessionState.GameMode;
            AutoOff.IsOn = SessionState.AutoPowerOff;
            CsNormal.IsChecked = SessionState.ControlNormal;
            CsReduction.IsChecked = SessionState.ControlReduction;
            CsAmbient.IsChecked = SessionState.ControlAmbient;
            _syncing = false;
        });
    }

    private void OnReadout(object sender, RoutedEventArgs e)
    {
        SessionState.SetHint("正在读取状态");
        _ = Task.Run(() =>
        {
            try
            {
                EdifierNative.Readout("basedevice");
                SessionState.SetHint("已请求读取状态");
            }
            catch (Exception ex)
            {
                SessionState.SetHint(ex.Message);
            }
        });
    }

    private void OnSetName(object sender, RoutedEventArgs e)
    {
        var name = NameBox.Text.Replace("\\", "\\\\").Replace("\"", "\\\"");
        Send("{\"op\":\"set_name\",\"name\":\"" + name + "\"}");
    }

    private void OnModeClick(object sender, RoutedEventArgs e)
    {
        if (_syncing || sender is not RadioButton rb || rb.Tag is not string mode)
        {
            return;
        }
        Send("{\"op\":\"set_noise_mode\",\"mode\":\"" + mode + "\"}");
    }

    private void OnAmbient(object sender, RoutedEventArgs e)
    {
        var v = (int)Ambient.Value;
        SessionState.AmbientVolume = v;
        Send("{\"op\":\"set_ambient_volume\",\"volume\":" + v + "}");
    }

    private void OnControl(object sender, RoutedEventArgs e)
    {
        if (_syncing)
        {
            return;
        }
        var n = (CsNormal.IsChecked == true ? 1 : 0)
            + (CsReduction.IsChecked == true ? 1 : 0)
            + (CsAmbient.IsChecked == true ? 1 : 0);
        if (n < 2)
        {
            SessionState.SetHint("至少选 2 项");
            return;
        }
        SessionState.ControlNormal = CsNormal.IsChecked == true;
        SessionState.ControlReduction = CsReduction.IsChecked == true;
        SessionState.ControlAmbient = CsAmbient.IsChecked == true;
        Send(
            "{\"op\":\"set_control_settings\",\"normal\":"
            + Bool(SessionState.ControlNormal)
            + ",\"reduction\":"
            + Bool(SessionState.ControlReduction)
            + ",\"ambient\":"
            + Bool(SessionState.ControlAmbient)
            + "}");
    }

    private void OnEffectClick(object sender, RoutedEventArgs e)
    {
        if (_syncing || sender is not RadioButton rb || rb.Tag is not string effect)
        {
            return;
        }
        Send("{\"op\":\"set_sound_effect\",\"effect\":\"" + effect + "\"}");
    }

    private void OnPrompt(object sender, RoutedEventArgs e)
    {
        var v = (int)Prompt.Value;
        SessionState.PromptVolume = v;
        Send("{\"op\":\"set_prompt_volume\",\"volume\":" + v + "}");
    }

    private void OnShutdownToggle(object sender, RoutedEventArgs e)
    {
        if (_syncing)
        {
            return;
        }
        SessionState.ShutdownOn = ShutdownSwitch.IsOn;
        if (ShutdownSwitch.IsOn)
        {
            Send("{\"op\":\"set_shutdown_timer\",\"minutes\":" + (int)ShutdownMinutes.Value + "}");
        }
        else
        {
            Send("""{"op":"disable_shutdown_timer"}""");
        }
    }

    private void OnShutdownSet(object sender, RoutedEventArgs e)
    {
        var m = (int)ShutdownMinutes.Value;
        SessionState.ShutdownMinutes = m;
        SessionState.ShutdownOn = true;
        _syncing = true;
        ShutdownSwitch.IsOn = true;
        _syncing = false;
        Send("{\"op\":\"set_shutdown_timer\",\"minutes\":" + m + "}");
    }

    private void OnLdacClick(object sender, RoutedEventArgs e)
    {
        if (_syncing || sender is not RadioButton rb || rb.Tag is not string mode)
        {
            return;
        }
        Send("{\"op\":\"set_ldac\",\"mode\":\"" + mode + "\"}");
    }

    private void OnGameMode(object sender, RoutedEventArgs e)
    {
        if (_syncing)
        {
            return;
        }
        SessionState.GameMode = GameMode.IsOn;
        Send("{\"op\":\"set_game_mode\",\"on\":" + Bool(GameMode.IsOn) + "}");
    }

    private void OnAutoOff(object sender, RoutedEventArgs e)
    {
        if (_syncing)
        {
            return;
        }
        SessionState.AutoPowerOff = AutoOff.IsOn;
        Send("{\"op\":\"set_auto_power_off\",\"on\":" + Bool(AutoOff.IsOn) + "}");
    }

    private void OnPlayback(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is string action)
        {
            Send("{\"op\":\"playback\",\"action\":\"" + action + "\"}");
        }
    }

    private void OnDisconnectHost(object sender, RoutedEventArgs e) =>
        Confirm("断开当前主机", "会给耳机发 CD, 当前正在播放的设备会掉线.", """{"op":"disconnect_host"}""");

    private void OnPowerOff(object sender, RoutedEventArgs e) =>
        Confirm("关机", "耳机会关机.", """{"op":"power_off"}""");

    private void OnRePair(object sender, RoutedEventArgs e) =>
        Confirm("重新配对", "耳机会进入配对.", """{"op":"re_pair"}""");

    private void OnFactoryReset(object sender, RoutedEventArgs e) =>
        Confirm("恢复出厂", "会清掉耳机设置.", """{"op":"factory_reset"}""");

    private async void Confirm(string title, string body, string json)
    {
        var dlg = new ContentDialog
        {
            Title = title,
            Content = body,
            PrimaryButtonText = "发送",
            CloseButtonText = "取消",
            XamlRoot = XamlRoot,
        };
        if (await dlg.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        Send(json);
    }

    private static string Bool(bool v) => v ? "true" : "false";

    private static void Send(string json)
    {
        try
        {
            EdifierNative.SendJson(json);
            SessionState.SetHint("已发送");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }
}
