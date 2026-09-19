using System.Text.Json;
using EdifierCtrl.Infrastructure;
using Microsoft.UI.Dispatching;

namespace EdifierCtrl.Native;

/// <summary>
/// 跨页面的已确认状态. 写入与 Changed 通知统一归并到 DispatcherQueue.
/// </summary>
public static class SessionState
{
    private static DispatcherQueue? _dispatcher;
    private static IReadOnlyList<HeadphoneDevice> _scanned = [];
    private static readonly List<ActivityEntry> ActivityList = [];
    private static string _audio = "unknown";
    private static string? _handoff;

    public static event Action? Changed;
    public static bool Ready { get; private set; }
    public static string? Operation { get; private set; }
    public static bool Busy => Operation is not null || HandoffActive;
    public static bool CanControl => Ready && Connected && !Busy;
    public static string CoreVersion { get; private set; } = "";
    public static bool Connected { get; private set; }
    public static string? Address { get; private set; }
    public static string? DeviceName { get; private set; }
    public static int? Battery { get; private set; }
    public static string Hint { get; private set; } = "扫描已配对的耳机, 点列表连接.";
    public static bool GroupJoined { get; private set; }
    public static string JoinedGroupName { get; private set; } = "";
    public static string? Holding { get; private set; }
    public static IReadOnlyList<GroupPeer> Peers { get; private set; } = [];
    public static IReadOnlyList<HeadphoneDevice> Devices { get; private set; } = [];
    public static IReadOnlyList<HeadphoneProfile> Profiles { get; private set; } = [];
    public static HeadphoneProfile SelectedProfile { get; private set; } = new() { DisplayName = "尚未加载机型" };
    public static IReadOnlyList<ActivityEntry> Activities { get; private set; } = [];
    public static string? Noise { get; private set; }
    public static string? Mac { get; private set; }
    public static string? Firmware { get; private set; }
    public static int? AmbientVolume { get; private set; }
    public static string? Effect { get; private set; }
    public static bool? GameMode { get; private set; }
    public static string? Ldac { get; private set; }
    public static int? PromptVolume { get; private set; }
    public static bool? ShutdownOn { get; private set; }
    public static int? ShutdownMinutes { get; private set; }
    public static bool? AutoPowerOff { get; private set; }
    public static bool? ControlNormal { get; private set; }
    public static bool? ControlReduction { get; private set; }
    public static bool? ControlAmbient { get; private set; }
    public static string? NoticeTitle { get; private set; }
    public static string? NoticeDetail { get; private set; }
    public static bool NoticeIsError { get; private set; }
    public static string? HandoffTitle { get; private set; }
    public static string? HandoffDetail { get; private set; }
    public static int HandoffStep { get; private set; }
    public static bool HandoffActive => _handoff is "requesting" or "waiting_peer" or "releasing" or "connecting" or "fallback_cd";
    internal static bool HandoffDone => _handoff == "done";

    public static string HeadphoneName => !string.IsNullOrWhiteSpace(DeviceName) ? DeviceName
        : Devices.FirstOrDefault(d => SameMac(d.Address, Address))?.DisplayName ?? "你的漫步者耳机";
    public static string AudioLabel => _audio switch
    {
        "connected" => "系统音频已就绪",
        "connecting" => "系统音频连接中",
        "disconnected" => "系统音频未连接",
        _ => "系统音频待确认",
    };

    public static bool Supports(string feature) => SelectedProfile.Supports(feature);
    public static string StatusLine() => $"{(Connected ? "控制通道已连接" : "控制通道未连接")}  {HeadphoneName}"
        + (Battery is int battery ? $"  |  电量 {battery}%" : "") + $"  |  {AudioLabel}";

    public static void ClearNotice() => Dispatch(() =>
    {
        NoticeTitle = null;
        NoticeDetail = null;
        NoticeIsError = false;
        Raise();
    });
    public static void ClearActivities() => Dispatch(() =>
    {
        ActivityList.Clear();
        Activities = [];
        Raise();
    });

    internal static void Attach(DispatcherQueue dispatcher) => _dispatcher = dispatcher;
    internal static void SetReady(bool ready) { Ready = ready; Raise(); }
    internal static void SetOperation(string? operation) { Operation = operation; Raise(); }
    internal static void Initialize(string version, IReadOnlyList<HeadphoneProfile> profiles, string selected, string address, string name)
    {
        CoreVersion = version;
        Profiles = profiles;
        SelectedProfile = profiles.FirstOrDefault(p => p.Id == selected) ?? profiles[0];
        Address = AppActions.TryNormalizeAddress(address);
        DeviceName = string.IsNullOrWhiteSpace(name) ? null : name;
        Ready = true;
        Record("耳机服务已启动", $"核心版本 {version}");
        Raise();
    }

    internal static void SelectProfile(HeadphoneProfile profile)
    {
        SelectedProfile = profile;
        ClearReadings();
        Raise();
    }

    internal static void SetDevices(IReadOnlyList<HeadphoneDevice> devices)
    {
        _scanned = devices;
        RebuildDevices();
        Raise();
    }

    internal static void SetConnected(bool connected, string? address = null, string? name = null)
    {
        if (!connected || !SameMac(Address, address)) ClearReadings();
        if (address is not null)
        {
            if (!SameMac(Address, address)) DeviceName = null;
            Address = AppActions.NormalizeAddress(address);
        }
        if (!string.IsNullOrWhiteSpace(name)) DeviceName = name;
        Connected = connected;
        Hint = connected ? "控制通道已连接" : "控制通道已断开";
        RebuildDevices();
        Raise();
    }

    internal static void SetGroupJoined(bool joined, string groupName = "")
    {
        JoinedGroupName = joined ? groupName : "";
        GroupJoined = joined;
        if (!joined)
        {
            Peers = [];
            Holding = null;
            SetHandoff(null);
        }
        RebuildDevices();
        Raise();
    }

    internal static void Stop()
    {
        Ready = false;
        Connected = false;
        Operation = null;
        ClearReadings();
        SetGroupJoined(false);
        Hint = "耳机服务已停止";
        Record(Hint);
        Raise();
    }

    internal static void Notify(string title, string detail, bool error = false)
    {
        NoticeTitle = title;
        NoticeDetail = detail;
        NoticeIsError = error;
        Hint = title;
        Record(title, detail, error);
        Raise();
    }

    internal static void Record(string title, string detail = "", bool error = false)
    {
        ActivityList.Insert(0, new ActivityEntry(DateTimeOffset.Now, title, detail, error));
        if (ActivityList.Count > 200) ActivityList.RemoveRange(200, ActivityList.Count - 200);
        Activities = ActivityList.ToArray();
        var message = string.IsNullOrWhiteSpace(detail) ? title : $"{title}: {detail}";
        if (error) AppLog.Error(message, "session"); else AppLog.Info(message, "session");
        Raise();
    }

    internal static void ApplySnapshot(NativeSnapshot snapshot)
    {
        if (GroupJoined && snapshot.Peers is not null)
        {
            Peers = snapshot.Peers.Select(peer => peer with { Holding = AppActions.TryNormalizeAddress(peer.Holding) })
                .GroupBy(peer => peer.Id, StringComparer.Ordinal).Select(group => group.Last())
                .OrderBy(peer => peer.DisplayName, StringComparer.CurrentCultureIgnoreCase).ToArray();
        }
        foreach (var item in snapshot.Events) ApplyEvent(item);
        Holding = GroupJoined ? snapshot.Holding : null;
        // Holding 来自平台确认的系统音频状态, 控制通道不参与推断.
        if (snapshot.Holding is not null) _audio = "connected";
        else if (_audio == "connected") _audio = "unknown";
        RebuildDevices();
        Raise();
    }

    private static void ApplyEvent(JsonElement root)
    {
        switch (root.GetProperty("kind").GetString())
        {
            case "bt_state":
                var connected = root.GetProperty("connected").GetBoolean();
                SetConnected(connected, NativeJson.Text(root, "address"));
                Record(connected ? "控制通道已连接" : "控制通道已断开");
                break;
            case "headset":
                if (Connected) ApplyHeadset(root.GetProperty("notification"));
                break;
            case "audio":
                _audio = NativeJson.Text(root, "state") ?? "unknown";
                if (_audio == "disconnected") Holding = null;
                break;
            case "handoff":
                if (!GroupJoined) break;
                var progress = root.GetProperty("progress");
                var kind = progress.GetProperty("kind").GetString();
                var reason = NativeJson.Text(progress, "reason");
                if (kind == "busy")
                {
                    Notify("已有交接正在进行", reason ?? "等待当前交接完成后再试.", true);
                    break;
                }
                SetHandoff(kind, reason);
                if (kind == "failed") Notify(HandoffTitle ?? "交接未完成", HandoffDetail ?? "", true);
                else Record(HandoffTitle ?? "交接状态已更新", HandoffDetail ?? "");
                break;
            case "message":
                if (NativeJson.Text(root, "text") is { Length: > 0 } text) Record("耳机服务", text);
                break;
        }
    }

    internal static void SetHandoff(string? kind, string? reason = null)
    {
        _handoff = kind;
        HandoffTitle = kind switch
        {
            "requesting" => "正在请求交接",
            "waiting_peer" => "等待另一台设备释放耳机",
            "releasing" => "正在将耳机交给另一台设备",
            "connecting" => "正在连接本机音频",
            "fallback_cd" => "正在尝试释放原连接",
            "done" => "交接已完成",
            "failed" => "交接未完成",
            _ => null,
        };
        HandoffDetail = reason ?? (kind switch
        {
            "requesting" => "请求已经发出, 等待两端确认.",
            "waiting_peer" => "保持两端应用运行并等待音频释放.",
            "releasing" => "正在释放本机系统音频连接.",
            "connecting" => "等待系统确认音频连接.",
            "fallback_cd" => "原连接未释放, 正在尝试耳机断连指令.",
            "done" => "系统已确认交接结果.",
            "failed" => "请检查两端蓝牙状态后重试.",
            _ => null,
        });
        HandoffStep = kind switch
        {
            "waiting_peer" or "releasing" or "fallback_cd" => 1,
            "connecting" => 2,
            "done" => 3,
            _ => 0,
        };
        Raise();
    }

    private static void ApplyHeadset(JsonElement notification)
    {
        switch (notification.GetProperty("kind").GetString())
        {
            case "battery":
                var battery = NativeJson.Integer(notification, "percent");
                Battery = battery is >= 0 and <= 100 ? battery : null;
                break;
            case "noise":
                Noise = NativeJson.Text(notification, "mode");
                AmbientVolume = NativeJson.Integer(notification, "ambient_volume");
                break;
            case "name": DeviceName = NativeJson.Text(notification, "name"); break;
            case "mac": Mac = AppActions.TryNormalizeAddress(NativeJson.Text(notification, "address")); break;
            case "firmware": Firmware = NativeJson.Text(notification, "version"); break;
            case "sound_effect": Effect = NativeJson.Text(notification, "effect"); break;
            case "game_mode": GameMode = NativeJson.Boolean(notification, "on"); break;
            case "ldac": Ldac = NativeJson.Text(notification, "mode"); break;
            case "prompt_volume": PromptVolume = NativeJson.Integer(notification, "volume"); break;
            case "shutdown_timer_enabled": ShutdownOn = NativeJson.Boolean(notification, "on"); break;
            case "shutdown_timer":
                ShutdownMinutes = NativeJson.Integer(notification, "minutes");
                ShutdownOn = true;
                break;
            case "auto_power_off": AutoPowerOff = NativeJson.Boolean(notification, "on"); break;
            case "control_settings":
                ControlNormal = NativeJson.Boolean(notification, "normal");
                ControlReduction = NativeJson.Boolean(notification, "reduction");
                ControlAmbient = NativeJson.Boolean(notification, "ambient");
                break;
        }
    }

    internal static void ClearReadings()
    {
        Battery = null;
        Mac = null;
        Firmware = null;
        Noise = null;
        AmbientVolume = null;
        Effect = null;
        GameMode = null;
        Ldac = null;
        PromptVolume = null;
        ShutdownOn = null;
        ShutdownMinutes = null;
        AutoPowerOff = null;
        ControlNormal = null;
        ControlReduction = null;
        ControlAmbient = null;
    }

    private static void RebuildDevices()
    {
        var map = new Dictionary<string, HeadphoneDevice>(StringComparer.Ordinal);
        foreach (var device in _scanned)
        {
            var address = AppActions.TryNormalizeAddress(device.Address);
            if (address is not null) map[address] = device with { Address = address };
        }
        foreach (var peer in Peers.Where(peer => peer.CanAudio && peer.Holding is not null))
        {
            var address = peer.Holding!;
            if (SameMac(Holding, address)) continue;
            var device = map.GetValueOrDefault(address) ?? new HeadphoneDevice { Address = address, Name = $"{peer.DisplayName} 的耳机" };
            map[address] = device with { PeerId = peer.Id, ActionText = "接管音频" };
        }
        Devices = map.Values.Select(device => device.PeerId is not null ? device : device with
            { ActionText = Connected && SameMac(Address, device.Address) ? "控制已连接" : "连接控制" })
            .OrderByDescending(device => IsEdifierName(device.Name))
            .ThenBy(device => device.DisplayName, StringComparer.CurrentCultureIgnoreCase).ToArray();
    }

    public static bool SameMac(string? first, string? second) =>
        AppActions.TryNormalizeAddress(first) is { } normalized && normalized == AppActions.TryNormalizeAddress(second);
    public static GroupPeer? HolderOf(string address) => Peers.FirstOrDefault(peer => SameMac(peer.Holding, address));
    public static bool IsEdifierName(string? name) => !string.IsNullOrWhiteSpace(name) &&
        (name.Contains("EDIFIER", StringComparison.OrdinalIgnoreCase) || name.Contains("漫步者")
        || name.StartsWith("W820", StringComparison.OrdinalIgnoreCase) || name.StartsWith("W200", StringComparison.OrdinalIgnoreCase));
    public static string NoiseLabel(string? mode) => mode switch
    {
        "normal" => "关闭", "reduction" => "降噪", "ambient" => "通透", _ => mode ?? "未知",
    };
    public static string EffectLabel(string? effect) => effect switch
    {
        "normal" => "标准", "pop" => "流行", "classical" => "古典", "rock" => "摇滚", _ => effect ?? "未知",
    };
    public static string LdacLabel(string? mode) => mode switch
    {
        "off" => "关闭", "rate48k" => "44.1k / 48k", "rate96k" => "96k", _ => mode ?? "未知",
    };

    private static void Dispatch(Action action)
    {
        if (_dispatcher is null || _dispatcher.HasThreadAccess) action();
        else if (!_dispatcher.TryEnqueue(() => action())) AppLog.Warn("窗口正在退出, 状态更新未入队.", "session");
    }
    private static void Raise()
    {
        if (Changed is not { } handlers) return;
        foreach (Action handler in handlers.GetInvocationList())
        {
            try { handler(); }
            catch (Exception error) { AppLog.Error($"状态视图更新失败: {error}", "ui"); }
        }
    }
}
