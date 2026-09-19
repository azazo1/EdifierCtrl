using System;
using System.Linq;
using EdifierCtrl.Desktop;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;

namespace EdifierCtrl.Pages;

public sealed partial class DevicePage : Page
{
    private bool _initialized;
    private bool _subscribed;
    private bool _scanning;
    private string _listKey = "";
    private string _listedKind = "rfcomm";

    public DevicePage()
    {
        InitializeComponent();
        _initialized = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private string Kind => (KindChoice.SelectedItem as ComboBoxItem)?.Tag as string ?? "rfcomm";
    private static bool CanAct => SessionState.Ready && !SessionState.Busy;

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (!_subscribed)
        {
            SessionState.Changed += OnState;
            _subscribed = true;
        }
        OnState();
        if (CanAct && SessionState.Devices.Count == 0)
        {
            await ScanAsync();
        }
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        SessionState.Changed -= OnState;
        _subscribed = false;
    }

    private void OnState()
    {
        if (!_initialized)
        {
            return;
        }
        ScanButton.IsEnabled = CanAct && !_scanning;
        ScanButton.Content = _scanning ? "正在刷新" : "刷新设备";
        DevicesList.IsEnabled = CanAct && !_scanning;
        KindChoice.IsEnabled = CanAct && !_scanning;
        AddressInput.IsEnabled = CanAct && !_scanning;
        ConnectAddressButton.IsEnabled = CanAct && !_scanning && PageUi.NormalizeAddress(AddressInput.Text) is not null;
        ListStatus.Text = _scanning ? "正在读取蓝牙设备..." : !string.IsNullOrWhiteSpace(SessionState.Operation) ? SessionState.Operation
            : Kind == "rfcomm" ? "经典蓝牙控制通道. 控制连接与系统音频连接分别管理." : "低功耗蓝牙控制通道";
        RefreshRows();
    }

    private void RefreshRows()
    {
        var search = SearchInput.Text.Trim();
        var next = SessionState.Devices
            .Where(device => string.IsNullOrEmpty(search) || device.DisplayName.Contains(search, StringComparison.OrdinalIgnoreCase)
                || device.Address.Contains(search, StringComparison.OrdinalIgnoreCase))
            .Select(device =>
            {
                var connected = SessionState.Connected && PageUi.SameAddress(device.Address, SessionState.Address);
                var occupied = !string.IsNullOrEmpty(device.PeerId);
                var peer = occupied ? SessionState.Peers.FirstOrDefault(item => item.Id == device.PeerId) : null;
                return new DeviceRow
                {
                    Address = device.Address,
                    Name = device.Name,
                    Kind = device.Kind,
                    DisplayName = device.DisplayName,
                    PeerId = device.PeerId,
                    Detail = connected ? "控制通道已连接" : device.ActionText,
                    Action = connected ? "断开控制" : occupied ? "接管到本机" : "连接",
                    Available = connected || !occupied || (SessionState.GroupJoined && peer?.CanAudio == true
                        && !PageUi.SameAddress(SessionState.Holding, device.Address)),
                    Connected = connected,
                };
            }).ToArray();
        var key = string.Join("\n", next.Select(row => $"{row.Address}\t{row.Kind}\t{row.DisplayName}\t{row.PeerId}\t{row.Detail}\t{row.Action}\t{row.Available}"));
        if (_listKey != key)
        {
            _listKey = key;
            DevicesList.ItemsSource = next;
        }
        EmptyCard.Visibility = PageUi.Visible(next.Length == 0);
        EmptyTitle.Text = _scanning ? "正在读取蓝牙设备" : string.IsNullOrEmpty(search) ? "还没有找到耳机" : "没有匹配的设备";
        EmptyDetail.Text = string.IsNullOrEmpty(search)
            ? "确认耳机已在 Windows 蓝牙设置中配对并打开, 然后刷新设备列表. 同组设备持有的耳机也会出现在这里."
            : "试试其他名称或蓝牙地址, 或清空搜索查看全部设备.";
    }

    private async System.Threading.Tasks.Task ScanAsync()
    {
        if (!CanAct || _scanning)
        {
            return;
        }
        _scanning = true;
        _listedKind = Kind;
        OnState();
        try
        {
            await AppActions.ScanAsync(_listedKind);
        }
        finally
        {
            _scanning = false;
            OnState();
        }
    }

    private async void OnScan(object sender, RoutedEventArgs e) => await ScanAsync();
    private void OnBluetooth(object sender, RoutedEventArgs e) => DesktopCommands.OpenBluetoothSettings();
    private void OnSearch(object sender, TextChangedEventArgs e)
    {
        if (_initialized)
        {
            RefreshRows();
        }
    }
    private void OnAddress(object sender, TextChangedEventArgs e) => OnState();

    private async void OnKind(object sender, SelectionChangedEventArgs e)
    {
        if (_initialized && CanAct)
        {
            await ScanAsync();
        }
    }

    private async void OnDevice(object sender, RoutedEventArgs e)
    {
        if (!CanAct || _scanning || sender is not Button { Tag: DeviceRow row } || !row.Available)
        {
            return;
        }
        if (row.Connected)
        {
            await AppActions.DisconnectAsync();
        }
        else if (!string.IsNullOrEmpty(row.PeerId))
        {
            await AppActions.ClaimPeerAsync(row.PeerId);
        }
        else
        {
            await AppActions.ConnectAsync(row.Address, row.Name, row.Kind);
        }
    }

    private async void OnConnectAddress(object sender, RoutedEventArgs e)
    {
        if (CanAct && !_scanning && PageUi.NormalizeAddress(AddressInput.Text) is not null)
        {
            await AppActions.ConnectAsync(AddressInput.Text.Trim(), kind: Kind);
        }
    }

    [Bindable]
    public sealed class DeviceRow
    {
        public string Address { get; set; } = "";
        public string Name { get; set; } = "";
        public string Kind { get; set; } = "rfcomm";
        public string DisplayName { get; set; } = "";
        public string? PeerId { get; set; }
        public string Detail { get; set; } = "";
        public string Action { get; set; } = "";
        public bool Available { get; set; }
        public bool Connected { get; set; }
    }
}
