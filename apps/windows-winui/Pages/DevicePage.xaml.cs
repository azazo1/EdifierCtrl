using System.Text.Json;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class DevicePage : Page
{
    public DevicePage()
    {
        InitializeComponent();
        try
        {
            EdifierNative.EnsureSession();
        }
        catch (Exception ex)
        {
            SessionState.SetHint("尚未加载 edifier_ffi.dll: " + ex.Message);
        }
    }

    private string KindValue()
    {
        if (Kind.SelectedItem is RadioButton rb && rb.Tag is string tag)
        {
            return tag;
        }
        return "rfcomm";
    }

    private void OnScan(object sender, RoutedEventArgs e)
    {
        var kind = KindValue();
        try
        {
            var json = EdifierNative.Scan(kind);
            var items = new List<DeviceItem>();
            using var doc = JsonDocument.Parse(json);
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var address = item.GetProperty("address").GetString() ?? "";
                var name = item.TryGetProperty("name", out var n) ? n.GetString() ?? "" : "";
                items.Add(new DeviceItem
                {
                    Address = address,
                    Name = string.IsNullOrWhiteSpace(name) ? "未命名耳机" : name,
                });
            }
            Devices.ItemsSource = items;
            SessionState.SetHint(items.Count == 0
                ? "没有发现设备. 请先在系统蓝牙里配对."
                : $"找到 {items.Count} 台, 点列表连接.");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private void OnDeviceClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is DeviceItem row)
        {
            Connect(row);
        }
    }

    private void Connect(DeviceItem row)
    {
        try
        {
            EdifierNative.Connect(row.Address, KindValue());
            SessionState.SetConnected(true, row.Address, row.Name);
            try
            {
                EdifierNative.SendJson("""{"op":"query_battery"}""");
            }
            catch
            {
                // 查电量失败不影响已连接.
            }
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private void OnDisconnect(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.Disconnect();
            SessionState.SetConnected(false, SessionState.Address, SessionState.DeviceName);
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private sealed class DeviceItem
    {
        public required string Address { get; init; }
        public required string Name { get; init; }
    }
}
