using System;
using System.Linq;
using System.Text;
using EdifierCtrl.Desktop;
using EdifierCtrl.Infrastructure;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace EdifierCtrl.Pages;

public sealed partial class ControlPage : Page
{
    private readonly ReadingDraft<int?> _ambientDraft = new();
    private readonly ReadingDraft<int?> _promptDraft = new();
    private readonly ReadingDraft<int?> _timerDraft = new();
    private readonly ReadingDraft<string> _nameDraft = new();
    private readonly ReadingDraft<(bool? Normal, bool? Reduction, bool? Ambient)> _controlsDraft = new();
    private bool _initialized;
    private bool _subscribed;
    private bool _syncing;
    private bool _dialogOpen;
    private string? _deviceKey;
    private string _profileKey = "";

    public ControlPage()
    {
        InitializeComponent();
        _initialized = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private static bool CanControl => SessionState.CanControl && !SessionState.Busy;

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (!_subscribed)
        {
            SessionState.Changed += OnState;
            AppPreferences.Changed += OnState;
            _subscribed = true;
        }
        OnState();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        SessionState.Changed -= OnState;
        AppPreferences.Changed -= OnState;
        _subscribed = false;
    }

    private void OnState()
    {
        if (!_initialized)
        {
            return;
        }
        _syncing = true;
        try
        {
            var connected = SessionState.Connected;
            var deviceKey = connected ? SessionState.Address ?? "connected" : null;
            if (_deviceKey != deviceKey)
            {
                _deviceKey = deviceKey;
                _ambientDraft.Reset();
                _promptDraft.Reset();
                _timerDraft.Reset();
                _nameDraft.Reset();
                _controlsDraft.Reset();
            }

            HeroName.Text = SessionState.HeadphoneName;
            HeroStatus.Text = connected ? "控制已连接" : "等待连接";
            BatteryText.Text = SessionState.Battery is int battery ? $"电量 {battery}%" : "电量: 等待耳机回报";
            HeroDetail.Text = connected ? SessionState.AudioLabel : "控制音效, 管理连接, 在设备之间自由切换.";
            HeroActions.Visibility = PageUi.Visible(connected);
            ConnectionEmpty.Visibility = PageUi.Visible(!connected);
            ConnectedContent.Visibility = PageUi.Visible(connected);
            MaintenanceExpander.Visibility = PageUi.Visible(connected);
            ReadoutButton.IsEnabled = RefreshButton.IsEnabled = CanControl;
            DisconnectButton.IsEnabled = connected && !SessionState.Busy;
            ConnectButton.IsEnabled = SessionState.Ready && !SessionState.Busy;
            ReconnectButton.Visibility = PageUi.Visible(!string.IsNullOrWhiteSpace(AppPreferences.Current.LastDeviceAddress));
            ReconnectButton.IsEnabled = SessionState.Ready && !SessionState.Busy;

            NoiseCard.Visibility = PageUi.Visible(SessionState.Supports("noise"));
            NoiseChoices.IsEnabled = CanControl;
            NoiseNormal.IsChecked = SessionState.Noise == "normal";
            NoiseReduction.IsChecked = SessionState.Noise == "reduction";
            NoiseAmbient.IsChecked = SessionState.Noise == "ambient";
            NoiseReading.Text = NoiseLabel(SessionState.Noise);
            var ambientSupported = SessionState.Supports("ambient_sound");
            NoiseAmbient.Visibility = PageUi.Visible(ambientSupported);
            AmbientColumn.Width = ambientSupported ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
            AmbientSection.Visibility = PageUi.Visible(ambientSupported && SessionState.Noise == "ambient");
            AmbientReading.Text = Reading(SessionState.AmbientVolume, "", "仅在环境声模式下生效");
            _ambientDraft.Synchronize(SessionState.AmbientVolume, PageUi.HasFocus(AmbientInput), value => AmbientInput.Value = value ?? double.NaN);

            PlaybackCard.Visibility = PageUi.Visible(SessionState.Supports("playback"));
            PlaybackColumn.Width = SessionState.Supports("playback") ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
            PlaybackActions.IsEnabled = CanControl;
            AudioReading.Text = SessionState.AudioLabel;
            GroupReading.Text = SessionState.GroupJoined ? $"交接组内 {SessionState.Peers.Count + 1} 台设备" : "尚未加入交接组";

            EffectExpander.Visibility = PageUi.Visible(SessionState.Supports("sound_effect"));
            EffectChoices.IsEnabled = CanControl;
            EffectNormal.IsChecked = SessionState.Effect == "normal";
            EffectPop.IsChecked = SessionState.Effect == "pop";
            EffectClassical.IsChecked = SessionState.Effect == "classical";
            EffectRock.IsChecked = SessionState.Effect == "rock";
            EffectReading.Text = SessionState.Effect switch
            {
                "normal" => "当前: 标准", "pop" => "当前: 流行", "classical" => "当前: 古典", "rock" => "当前: 摇滚",
                null or "" => "等待耳机回报", _ => "当前: " + SessionState.Effect,
            };

            SettingsExpander.Visibility = PageUi.Visible(new[] { "game_mode", "auto_power_off", "prompt_volume", "shutdown_timer", "control_settings", "ldac", "name" }.Any(SessionState.Supports));
            GameSection.Visibility = PageUi.Visible(SessionState.Supports("game_mode"));
            GameReading.Text = ToggleReading(SessionState.GameMode, "适合游戏与视频通话");
            GameSwitch.IsOn = SessionState.GameMode == true;
            GameSwitch.OffContent = SessionState.GameMode.HasValue ? "关闭" : "未读取";
            GameSwitch.IsEnabled = CanControl;
            AutoOffSection.Visibility = PageUi.Visible(SessionState.Supports("auto_power_off"));
            AutoOffReading.Text = ToggleReading(SessionState.AutoPowerOff, "由耳机管理待机耗电");
            AutoOffSwitch.IsOn = SessionState.AutoPowerOff == true;
            AutoOffSwitch.OffContent = SessionState.AutoPowerOff.HasValue ? "关闭" : "未读取";
            AutoOffSwitch.IsEnabled = CanControl;

            PromptSection.Visibility = PageUi.Visible(SessionState.Supports("prompt_volume"));
            PromptReading.Text = Reading(SessionState.PromptVolume, "", "耳机开关机与模式切换提示");
            _promptDraft.Synchronize(SessionState.PromptVolume, PageUi.HasFocus(PromptInput), value => PromptInput.Value = value ?? double.NaN);
            TimerSection.Visibility = PageUi.Visible(SessionState.Supports("shutdown_timer"));
            TimerReading.Text = SessionState.ShutdownOn switch
            {
                true => SessionState.ShutdownMinutes is int minutes ? $"已开启, 耳机回报 {minutes} 分钟" : "已开启, 等待倒计时回报",
                false => "定时关机未开启", _ => "等待耳机回报",
            };
            _timerDraft.Synchronize(SessionState.ShutdownMinutes, PageUi.HasFocus(TimerInput), value => TimerInput.Value = value ?? double.NaN);
            TimerDisable.IsEnabled = CanControl && SessionState.ShutdownOn == true;

            ControlsSection.Visibility = PageUi.Visible(SessionState.Supports("control_settings"));
            ControlAmbient.Visibility = PageUi.Visible(ambientSupported);
            _controlsDraft.Synchronize((SessionState.ControlNormal, SessionState.ControlReduction, SessionState.ControlAmbient), PageUi.HasFocus(ControlsSection), ApplyControlReadings);
            ControlsReading.Text = SessionState.ControlNormal.HasValue && SessionState.ControlReduction.HasValue && (!ambientSupported || SessionState.ControlAmbient.HasValue)
                ? "已读取按键模式. 修改后点击应用." : "等待耳机回报. 勾选项是待应用草稿.";
            ControlNormal.IsEnabled = ControlReduction.IsEnabled = ControlAmbient.IsEnabled = CanControl;

            LdacSection.Visibility = PageUi.Visible(SessionState.Supports("ldac"));
            LdacChoice.SelectedIndex = SessionState.Ldac switch { "off" => 1, "rate48k" => 2, "rate96k" => 3, _ => 0 };
            LdacChoice.IsEnabled = CanControl && !_dialogOpen;
            NameSection.Visibility = PageUi.Visible(SessionState.Supports("name"));
            _nameDraft.Synchronize(connected ? SessionState.DeviceName ?? "" : "", PageUi.HasFocus(NameInput), value => NameInput.Text = value);
            NameInput.PlaceholderText = string.IsNullOrWhiteSpace(SessionState.DeviceName) ? "等待耳机回报, 或输入新名称" : "输入耳机名称";
            NameHint.Text = $"最多 {SessionState.SelectedProfile.MaxNameLen} 个 UTF-8 字节. 新名称可能需要重新配对后显示.";

            FirmwareReading.Text = SessionState.Firmware ?? "等待耳机回报";
            AddressReading.Text = SessionState.Mac ?? SessionState.Address ?? "等待耳机回报";
            var profilesKey = string.Join("|", SessionState.Profiles.Select(profile => profile.Id + ":" + profile.DisplayName));
            if (_profileKey != profilesKey)
            {
                _profileKey = profilesKey;
                ProfileChoice.ItemsSource = SessionState.Profiles;
            }
            ProfileChoice.SelectedValue = SessionState.SelectedProfile.Id;
            ProfileChoice.IsEnabled = SessionState.Ready && !SessionState.Busy;
            MaintenanceActions.IsEnabled = CanControl && !_dialogOpen;
            UpdateDraftButtons();
        }
        finally
        {
            _syncing = false;
        }
    }

    private static string Reading(int? value, string unit, string detail) => value is int number
        ? $"当前: {number}{unit}. {detail}" : "等待耳机回报. " + detail;

    private static string ToggleReading(bool? value, string detail) => value.HasValue
        ? $"{(value.Value ? "已开启" : "已关闭")}. {detail}" : "等待耳机回报. " + detail;

    private static string NoiseLabel(string? value) => value switch
    {
        "normal" => "当前: 标准", "reduction" => "当前: 主动降噪", "ambient" => "当前: 环境声",
        null or "" => "等待耳机回报", _ => "当前: " + value,
    };

    private void UpdateDraftButtons()
    {
        if (!_initialized)
        {
            return;
        }
        AmbientInput.IsEnabled = PromptInput.IsEnabled = TimerInput.IsEnabled = NameInput.IsEnabled = CanControl;
        AmbientApply.IsEnabled = CanControl && ValidNumber(AmbientInput, -3, 3);
        PromptApply.IsEnabled = CanControl && ValidNumber(PromptInput, 0, 15);
        TimerApply.IsEnabled = CanControl && ValidNumber(TimerInput, 1, 180);
        NameApply.IsEnabled = CanControl && !string.IsNullOrWhiteSpace(NameInput.Text)
            && Encoding.UTF8.GetByteCount(NameInput.Text.Trim()) <= SessionState.SelectedProfile.MaxNameLen;
        var ambientSupported = SessionState.Supports("ambient_sound");
        var count = (ControlNormal.IsChecked == true ? 1 : 0) + (ControlReduction.IsChecked == true ? 1 : 0)
            + (ambientSupported && ControlAmbient.IsChecked == true ? 1 : 0);
        var changed = ControlNormal.IsChecked != SessionState.ControlNormal || ControlReduction.IsChecked != SessionState.ControlReduction
            || (ambientSupported && ControlAmbient.IsChecked != SessionState.ControlAmbient);
        ControlsApply.IsEnabled = CanControl && changed && count >= 2
            && ControlNormal.IsChecked.HasValue && ControlReduction.IsChecked.HasValue && (!ambientSupported || ControlAmbient.IsChecked.HasValue);
        ControlsReset.IsEnabled = CanControl && changed;
    }

    private static bool ValidNumber(NumberBox input, int min, int max) =>
        double.IsFinite(input.Value) && input.Value >= min && input.Value <= max && input.Value == Math.Truncate(input.Value);

    private void ApplyControlReadings((bool? Normal, bool? Reduction, bool? Ambient) value)
    {
        ControlNormal.IsChecked = value.Normal;
        ControlReduction.IsChecked = value.Reduction;
        ControlAmbient.IsChecked = value.Ambient;
    }

    private void OnDraftValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (!_syncing)
        {
            UpdateDraftButtons();
        }
    }

    private void OnNameDraft(object sender, TextChangedEventArgs e)
    {
        if (!_syncing)
        {
            UpdateDraftButtons();
        }
    }

    private void OnControlDraft(object sender, RoutedEventArgs e)
    {
        if (_syncing || !_initialized)
        {
            return;
        }
        if (sender is CheckBox { IsChecked: null } checkBox)
        {
            checkBox.IsChecked = false;
        }
        UpdateDraftButtons();
    }

    private void OnResetControls(object sender, RoutedEventArgs e)
    {
        ApplyControlReadings((SessionState.ControlNormal, SessionState.ControlReduction, SessionState.ControlAmbient));
        UpdateDraftButtons();
    }

    private async void OnApplyControls(object sender, RoutedEventArgs e)
    {
        if (!ControlsApply.IsEnabled)
        {
            return;
        }
        await AppActions.SendAsync("set_control_settings", new
        {
            normal = ControlNormal.IsChecked == true,
            reduction = ControlReduction.IsChecked == true,
            ambient = SessionState.Supports("ambient_sound") && ControlAmbient.IsChecked == true,
        }, "设置按键模式", "query_control_settings");
    }

    private async void OnReadout(object sender, RoutedEventArgs e)
    {
        if (CanControl)
        {
            await AppActions.ReadoutAsync();
        }
    }

    private async void OnDisconnect(object sender, RoutedEventArgs e)
    {
        if (SessionState.Connected && !SessionState.Busy)
        {
            await AppActions.DisconnectAsync();
        }
    }

    private void OnDevices(object sender, RoutedEventArgs e) => DesktopCommands.Navigate("devices");
    private void OnHandoff(object sender, RoutedEventArgs e) => DesktopCommands.Navigate("handoff");

    private async void OnReconnect(object sender, RoutedEventArgs e)
    {
        if (SessionState.Ready && !SessionState.Busy && !string.IsNullOrWhiteSpace(AppPreferences.Current.LastDeviceAddress))
        {
            await AppActions.ConnectAsync(AppPreferences.Current.LastDeviceAddress, AppPreferences.Current.LastDeviceName);
        }
    }

    private async void OnNoise(object sender, RoutedEventArgs e)
    {
        if (_syncing || !CanControl || sender is not ToggleButton { Tag: string mode })
        {
            return;
        }
        OnState();
        await AppActions.SendAsync("set_noise_mode", new { mode }, "切换聆听模式", "query_noise");
    }

    private async void OnEffect(object sender, RoutedEventArgs e)
    {
        if (_syncing || !CanControl || sender is not ToggleButton { Tag: string effect })
        {
            return;
        }
        OnState();
        await AppActions.SendAsync("set_sound_effect", new { effect }, "切换声音风格", "query_sound_effect");
    }

    private async void OnAmbient(object sender, RoutedEventArgs e)
    {
        if (CanControl && ValidNumber(AmbientInput, -3, 3))
        {
            await AppActions.SendAsync("set_ambient_volume", new { volume = (int)AmbientInput.Value }, "设置环境声强度", "query_noise");
        }
    }

    private async void OnPrompt(object sender, RoutedEventArgs e)
    {
        if (CanControl && ValidNumber(PromptInput, 0, 15))
        {
            await AppActions.SendAsync("set_prompt_volume", new { volume = (int)PromptInput.Value }, "设置提示音量", "query_prompt_volume");
        }
    }

    private async void OnTimer(object sender, RoutedEventArgs e)
    {
        if (CanControl && ValidNumber(TimerInput, 1, 180))
        {
            await AppActions.SendAsync("set_shutdown_timer", new { minutes = (int)TimerInput.Value }, "设置定时关机", "query_shutdown_timer");
        }
    }

    private async void OnDisableTimer(object sender, RoutedEventArgs e)
    {
        if (CanControl && SessionState.ShutdownOn == true)
        {
            await AppActions.SendAsync("disable_shutdown_timer", label: "关闭定时关机", query: "query_shutdown_timer");
        }
    }

    private async void OnSetName(object sender, RoutedEventArgs e)
    {
        if (NameApply.IsEnabled)
        {
            await AppActions.SendAsync("set_name", new { name = NameInput.Text.Trim() }, "更新耳机名称", "query_name");
        }
    }

    private async void OnGame(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing || !CanControl)
        {
            return;
        }
        var on = GameSwitch.IsOn;
        OnState();
        await AppActions.SendAsync("set_game_mode", new { on }, "设置游戏模式", "query_game_mode");
    }

    private async void OnAutoOff(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing || !CanControl)
        {
            return;
        }
        var on = AutoOffSwitch.IsOn;
        OnState();
        await AppActions.SendAsync("set_auto_power_off", new { on }, "设置自动关机", "query_auto_power_off");
    }

    private async void OnPlayback(object sender, RoutedEventArgs e)
    {
        if (CanControl && sender is Button { Tag: string action })
        {
            await AppActions.SendAsync("playback", new { action }, "播放控制");
        }
    }

    private async void OnProfile(object sender, SelectionChangedEventArgs e)
    {
        if (!_initialized || _syncing || !SessionState.Ready || SessionState.Busy || ProfileChoice.SelectedValue is not string id)
        {
            return;
        }
        await AppActions.SelectProfileAsync(id);
    }

    private async void OnLdac(object sender, SelectionChangedEventArgs e)
    {
        if (!_initialized || _syncing || !CanControl || _dialogOpen || LdacChoice.SelectedItem is not ComboBoxItem { Tag: string mode }
            || mode == "unknown" || mode == SessionState.Ldac)
        {
            return;
        }
        _dialogOpen = true;
        OnState();
        try
        {
            if (await PageUi.ConfirmAsync(this, "更改 LDAC 设置", "耳机可能中断蓝牙连接, 并关闭定时关机. 此设置不代表 Windows 已支持 LDAC 音频输出.", "更改设置") && CanControl)
            {
                await AppActions.SendAsync("set_ldac", new { mode }, "设置 LDAC", "query_ldac");
            }
        }
        finally
        {
            _dialogOpen = false;
            OnState();
        }
    }

    private async void OnMaintenance(object sender, RoutedEventArgs e)
    {
        if (!CanControl || _dialogOpen || sender is not Button { Tag: string operation, Content: string title })
        {
            return;
        }
        var detail = operation switch
        {
            "disconnect_host" => "耳机将断开当前主机的蓝牙连接. 日常切换设备请优先使用交接功能.",
            "power_off" => "耳机将关机, 当前播放会立即停止.",
            "re_pair" => "耳机将进入配对模式并中断当前连接.",
            "factory_reset" => "这会清除耳机设置和配对记录, 之后需要在各台设备上重新配对.",
            _ => "此操作会中断当前连接.",
        };
        _dialogOpen = true;
        OnState();
        try
        {
            if (await PageUi.ConfirmAsync(this, title, detail, title) && CanControl)
            {
                await AppActions.SendAsync(operation, label: title);
            }
        }
        finally
        {
            _dialogOpen = false;
            OnState();
        }
    }
}
