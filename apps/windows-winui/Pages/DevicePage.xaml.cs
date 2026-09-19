using System.Text.Json;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace EdifierCtrl.Pages;

public sealed partial class DevicePage : Page
{
    private string _kind = "rfcomm";
    private EventPump? _pump;

    public DevicePage()
    {
        InitializeComponent();
        try
        {
            EdifierNative.EnsureSession();
            Status.Text = "核心库 " + EdifierNative.Version();
        }
        catch (Exception ex)
        {
            Status.Text = "尚未加载 edifier_ffi.dll: " + ex.Message;
        }
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _pump = new EventPump(ev => Events.Text = ev + "\n" + Events.Text);
        _pump.Start();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        _pump?.Stop();
        _pump = null;
    }

    private void OnScanRfcomm(object sender, RoutedEventArgs e) => Scan("rfcomm");

    private void OnScanBle(object sender, RoutedEventArgs e) => Scan("ble");

    private void Scan(string kind)
    {
        _kind = kind;
        try
        {
            var json = EdifierNative.Scan(kind);
            Devices.Items.Clear();
            using var doc = JsonDocument.Parse(json);
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var address = item.GetProperty("address").GetString() ?? "";
                var name = item.TryGetProperty("name", out var n) ? n.GetString() : "";
                Devices.Items.Add($"{address}  {name}");
            }
            Status.Text = Devices.Items.Count == 0 ? "没有发现设备. 请先在系统里配对." : "选中后连接.";
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }

    private void OnDeviceClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is string row)
        {
            ConnectRow(row);
        }
    }

    private void OnConnect(object sender, RoutedEventArgs e)
    {
        if (Devices.SelectedItem is not string row)
        {
            Status.Text = "先选一个设备.";
            return;
        }
        ConnectRow(row);
    }

    private void ConnectRow(string row)
    {
        var address = row.Split(' ', 2, StringSplitOptions.RemoveEmptyEntries)[0];
        try
        {
            EdifierNative.Connect(address, _kind);
            Status.Text = "已连接 " + address;
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }

    private void OnDisconnect(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.Disconnect();
            Status.Text = "已断开控制通道.";
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }

    private void OnReadout(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.Readout("basedevice");
            Status.Text = "已发送读状态命令.";
        }
        catch (Exception ex)
        {
            Status.Text = ex.Message;
        }
    }
}
