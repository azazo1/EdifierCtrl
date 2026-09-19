using System;
using System.Linq;
using EdifierCtrl.Desktop;
using EdifierCtrl.Infrastructure;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;

namespace EdifierCtrl.Pages;

public sealed partial class GroupPage : Page
{
    private bool _initialized;
    private bool _subscribed;
    private bool _syncing;
    private string _peerKey = "";

    public GroupPage()
    {
        InitializeComponent();
        GroupInput.Text = AppPreferences.Current.GroupName;
        _initialized = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private static bool CanAct => SessionState.Ready && !SessionState.Busy && !SessionState.HandoffActive;

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (!_subscribed)
        {
            SessionState.Changed += OnState;
            AppPreferences.Changed += OnPreferences;
            _subscribed = true;
        }
        OnPreferences();
        OnState();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        SessionState.Changed -= OnState;
        AppPreferences.Changed -= OnPreferences;
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
            RememberChoice.IsChecked = AppPreferences.Current.RememberGroup;
            AutoJoinChoice.IsChecked = AppPreferences.Current.AutoJoinGroup;
            PreferenceError.IsOpen = !string.IsNullOrEmpty(AppPreferences.Current.SaveError);
            PreferenceError.Message = AppPreferences.Current.SaveError ?? "";
        }
        finally
        {
            _syncing = false;
        }
        UpdateForm();
    }

    private void OnState()
    {
        if (!_initialized)
        {
            return;
        }
        var joined = SessionState.GroupJoined;
        GroupTitle.Text = joined ? "设备已在同一交接组" : "把你的设备连在一起";
        JoinForm.Visibility = PageUi.Visible(!joined);
        JoinedPanel.Visibility = PageUi.Visible(joined);
        JoinedGroupName.Text = joined ? "组名: " + SessionState.JoinedGroupName : "";
        MembersSection.Visibility = PageUi.Visible(joined);
        LeaveButton.IsEnabled = CanAct;
        LocalTitle.Text = Environment.MachineName + " / 本机";
        LocalHolding.Text = string.IsNullOrWhiteSpace(SessionState.Holding) ? "尚未确认持有耳机音频" : "正在持有 " + SessionState.Holding;
        LocalAudio.Text = "Windows / " + SessionState.AudioLabel;
        UpdateForm();
        RefreshPeers();

        var hasProgress = !string.IsNullOrWhiteSpace(SessionState.HandoffTitle);
        ProgressCard.Visibility = PageUi.Visible(hasProgress);
        ProgressTitle.Text = SessionState.HandoffTitle ?? "";
        ProgressDetail.Text = SessionState.HandoffDetail ?? "";
        HandoffSpinner.IsActive = SessionState.HandoffActive;
        HandoffSpinner.Visibility = PageUi.Visible(SessionState.HandoffActive);
        StepRequest.Opacity = SessionState.HandoffStep >= 0 ? 1 : 0.15;
        StepRelease.Opacity = SessionState.HandoffStep >= 1 ? 1 : 0.15;
        StepConnect.Opacity = SessionState.HandoffStep >= 2 ? 1 : 0.15;
        StepDone.Opacity = SessionState.HandoffStep >= 3 ? 1 : 0.15;
        HandoffHelp.Visibility = PageUi.Visible(hasProgress && !SessionState.HandoffActive && SessionState.HandoffStep < 3);
    }

    private void UpdateForm()
    {
        if (!_initialized)
        {
            return;
        }
        GroupInput.IsEnabled = RememberChoice.IsEnabled = CanAct && !SessionState.GroupJoined;
        AutoJoinChoice.IsEnabled = CanAct && RememberChoice.IsChecked == true && !SessionState.GroupJoined;
        JoinButton.IsEnabled = CanAct && !SessionState.GroupJoined && !string.IsNullOrWhiteSpace(GroupInput.Text);
        AddressInput.IsEnabled = CanAct && SessionState.GroupJoined;
        ClaimButton.IsEnabled = CanAct && SessionState.GroupJoined && PageUi.NormalizeAddress(AddressInput.Text) is not null
            && !PageUi.SameAddress(AddressInput.Text, SessionState.Holding);
        PeersList.IsEnabled = CanAct && SessionState.GroupJoined;
    }

    private void RefreshPeers()
    {
        var next = SessionState.Peers.Select(peer =>
        {
            var holding = !string.IsNullOrWhiteSpace(peer.Holding);
            var alreadyHeld = PageUi.SameAddress(peer.Holding, SessionState.Holding);
            return new PeerRow
            {
                Id = peer.Id,
                DisplayName = peer.DisplayName,
                PlatformVersion = (string.IsNullOrWhiteSpace(peer.Platform) ? "平台未回报" : peer.Platform)
                    + " / " + (string.IsNullOrWhiteSpace(peer.AppVersion) ? "版本未回报" : peer.AppVersion),
                AudioCapability = peer.CanAudio ? "支持音频交接" : "不支持音频交接",
                HoldingText = holding ? "正在持有 " + peer.Holding : "当前没有持有耳机",
                Available = peer.CanAudio && holding && !alreadyHeld,
                Action = alreadyHeld ? "本机已持有" : "接管到本机",
            };
        }).ToArray();
        var key = string.Join("\n", next.Select(peer => $"{peer.Id}\t{peer.DisplayName}\t{peer.PlatformVersion}\t{peer.AudioCapability}\t{peer.HoldingText}\t{peer.Available}"));
        if (_peerKey != key)
        {
            _peerKey = key;
            PeersList.ItemsSource = next;
        }
        MembersEmpty.Visibility = PageUi.Visible(next.Length == 0);
    }

    private void OnGroupDraft(object sender, TextChangedEventArgs e) => UpdateForm();
    private void OnAddressDraft(object sender, TextChangedEventArgs e) => UpdateForm();

    private void OnRemember(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing)
        {
            return;
        }
        var preferences = AppPreferences.Current;
        preferences.RememberGroup = RememberChoice.IsChecked == true;
        if (!preferences.RememberGroup)
        {
            preferences.GroupName = "";
            preferences.AutoJoinGroup = false;
        }
        preferences.Save();
    }

    private void OnAutoJoin(object sender, RoutedEventArgs e)
    {
        if (!_initialized || _syncing)
        {
            return;
        }
        AppPreferences.Current.AutoJoinGroup = RememberChoice.IsChecked == true && AutoJoinChoice.IsChecked == true;
        AppPreferences.Current.Save();
    }

    private async void OnJoin(object sender, RoutedEventArgs e)
    {
        if (CanAct && !SessionState.GroupJoined && !string.IsNullOrWhiteSpace(GroupInput.Text))
        {
            await AppActions.JoinAsync(GroupInput.Text, RememberChoice.IsChecked == true);
        }
    }

    private async void OnLeave(object sender, RoutedEventArgs e)
    {
        if (CanAct && SessionState.GroupJoined)
        {
            await AppActions.LeaveAsync();
        }
    }

    private async void OnPeer(object sender, RoutedEventArgs e)
    {
        if (!CanAct || !SessionState.GroupJoined || sender is not Button { Tag: string id })
        {
            return;
        }
        var peer = SessionState.Peers.FirstOrDefault(item => item.Id == id);
        if (peer?.CanAudio == true && !string.IsNullOrWhiteSpace(peer.Holding) && !PageUi.SameAddress(peer.Holding, SessionState.Holding))
        {
            await AppActions.ClaimPeerAsync(id);
        }
    }

    private async void OnClaim(object sender, RoutedEventArgs e)
    {
        if (ClaimButton.IsEnabled)
        {
            await AppActions.ClaimAsync(AddressInput.Text.Trim());
        }
    }

    private void OnBluetooth(object sender, RoutedEventArgs e) => DesktopCommands.OpenBluetoothSettings();

    [Bindable]
    public sealed class PeerRow
    {
        public string Id { get; set; } = "";
        public string DisplayName { get; set; } = "";
        public string PlatformVersion { get; set; } = "";
        public string AudioCapability { get; set; } = "";
        public string HoldingText { get; set; } = "";
        public string Action { get; set; } = "";
        public bool Available { get; set; }
    }
}
