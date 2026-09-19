using System.Text.Json;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class DevicePage : Page
{
    private readonly List<DeviceItem> _scanned = [];

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
        Loaded += (_, _) =>
        {
            SessionState.Changed += OnState;
            Scan();
        };
        Unloaded += (_, _) => SessionState.Changed -= OnState;
    }

    private string KindValue()
    {
        if (Kind.SelectedItem is RadioButton rb && rb.Tag is string tag)
        {
            return tag;
        }
        return "rfcomm";
    }

    private void OnState()
    {
        DispatcherQueue.TryEnqueue(ShowDevices);
    }

    private void OnScan(object sender, RoutedEventArgs e) => Scan();

    private void Scan()
    {
        var kind = KindValue();
        try
        {
            var json = EdifierNative.Scan(kind);
            _scanned.Clear();
            using var doc = JsonDocument.Parse(json);
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var address = item.GetProperty("address").GetString() ?? "";
                var name = item.TryGetProperty("name", out var n) ? n.GetString() ?? "" : "";
                if (kind != "rfcomm" && !SessionState.IsEdifierName(name))
                {
                    continue;
                }
                _scanned.Add(new DeviceItem
                {
                    Address = address,
                    Name = string.IsNullOrWhiteSpace(name) ? "未命名耳机" : name,
                });
            }
            ShowDevices();
            SessionState.SetHint(_scanned.Count == 0 && !SessionState.Peers.Any(p => !string.IsNullOrEmpty(p.Holding))
                ? "没有发现漫步者耳机. 请先在系统蓝牙里配对."
                : $"找到 {Devices.Items.Count} 台.");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private void ShowDevices()
    {
        var map = new Dictionary<string, DeviceItem>(StringComparer.OrdinalIgnoreCase);
        foreach (var row in _scanned)
        {
            map[Norm(row.Address)] = Decorate(row.Address, row.Name);
        }
        foreach (var peer in SessionState.Peers)
        {
            if (string.IsNullOrEmpty(peer.Holding) || SessionState.SameMac(SessionState.Holding, peer.Holding))
            {
                continue;
            }
            var key = Norm(peer.Holding);
            if (map.ContainsKey(key))
            {
                continue;
            }
            map[key] = new DeviceItem
            {
                Address = peer.Holding,
                Name = "占用中的耳机",
                PeerId = peer.Id,
                Action = "被 " + peer.Host + " 占用 · 点按接管",
            };
        }
        Devices.ItemsSource = map.Values.ToList();
    }

    private static DeviceItem Decorate(string address, string name)
    {
        var holder = SessionState.HolderOf(address);
        var self = SessionState.SameMac(SessionState.Holding, address);
        var occupied = holder != null && !self;
        return new DeviceItem
        {
            Address = address,
            Name = name,
            PeerId = occupied ? holder!.Id : null,
            Action = occupied
                ? "被 " + holder!.Host + " 占用 · 点按接管"
                : self ? "本机持有 · 点按连接" : "点按连接",
        };
    }

    private static string Norm(string addr) => new string(addr.Where(char.IsLetterOrDigit).ToArray());

    private void OnDeviceClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not DeviceItem row)
        {
            return;
        }
        if (!string.IsNullOrEmpty(row.PeerId))
        {
            try
            {
                EdifierNative.GroupClaimPeer(row.PeerId);
                SessionState.SetHint("已向对端请求接管 " + row.Name);
            }
            catch (Exception ex)
            {
                SessionState.SetHint(ex.Message);
            }
            return;
        }
        Connect(row);
    }

    private void Connect(DeviceItem row)
    {
        var kind = KindValue();
        SessionState.SetHint("正在连接 " + row.Name);
        _ = Task.Run(() =>
        {
            try
            {
                EdifierNative.Connect(row.Address, kind);
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
        });
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
        public string? PeerId { get; init; }
        public string Action { get; init; } = "点按连接";
    }
}
