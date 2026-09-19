using System.Text.Json;
using Windows.Storage;

namespace EdifierCtrl.Native;

/// <summary>
/// 跨页面的连接 / 组 / 电量状态, 文案给用户看.
/// </summary>
internal static class SessionState
{
    public static event Action? Changed;

    public static bool Connected { get; private set; }
    public static string? Address { get; set; }
    public static string? DeviceName { get; set; }
    public static int? Battery { get; private set; }
    public static string Hint { get; private set; } = "扫描已配对的耳机, 点列表连接.";
    public static bool GroupJoined { get; set; }
    public static string? Holding { get; set; }
    public static IReadOnlyList<GroupPeer> Peers { get; private set; } = [];
    public static string? Noise { get; set; }
    public static string? Mac { get; private set; }
    public static string? Firmware { get; private set; }
    public static int AmbientVolume { get; set; }
    public static string? Effect { get; set; }
    public static bool GameMode { get; set; }
    public static string? Ldac { get; set; }
    public static int PromptVolume { get; set; } = 7;
    public static bool ShutdownOn { get; set; }
    public static int ShutdownMinutes { get; set; } = 5;
    public static bool AutoPowerOff { get; set; }
    public static bool ControlNormal { get; set; } = true;
    public static bool ControlReduction { get; set; } = true;
    public static bool ControlAmbient { get; set; } = true;

    public static string StatusLine()
    {
        var name = string.IsNullOrWhiteSpace(DeviceName) ? Address ?? "未连接耳机" : DeviceName;
        var bat = Battery is int pct ? $"  ·  电量 {pct}%" : "";
        var link = Connected ? "已连接" : "未连接";
        var hold = string.IsNullOrEmpty(Holding) ? "" : $"  ·  持有 {Holding}";
        var group = GroupJoined ? "  ·  已入组" : "";
        return $"{link}  {name}{bat}{hold}{group}";
    }

    public static void SetHint(string text)
    {
        Hint = text;
        Raise();
    }

    public static void SetConnected(bool connected, string? address, string? name)
    {
        Connected = connected;
        if (!string.IsNullOrEmpty(address))
        {
            Address = address;
        }
        if (!string.IsNullOrEmpty(name))
        {
            DeviceName = name;
        }
        Hint = connected ? "控制通道已连接" : "控制通道已断开";
        Raise();
    }

    public static void ApplyEvent(string raw)
    {
        try
        {
            using var doc = JsonDocument.Parse(raw);
            var root = doc.RootElement;
            var kind = root.GetProperty("kind").GetString();
            switch (kind)
            {
                case "empty":
                    return;
                case "bt_state":
                    Connected = root.GetProperty("connected").GetBoolean();
                    if (root.TryGetProperty("address", out var addr) && addr.ValueKind == JsonValueKind.String)
                    {
                        Address = addr.GetString();
                    }
                    Hint = Connected ? "控制通道已连接" : "控制通道已断开";
                    break;
                case "headset":
                    ApplyHeadset(root.GetProperty("notification"));
                    break;
                case "handoff":
                    Hint = HandoffText(root.GetProperty("progress"));
                    break;
                case "audio":
                    Hint = root.TryGetProperty("state", out var st) ? AudioText(st.GetString()) : Hint;
                    break;
                case "message":
                    if (root.TryGetProperty("text", out var text) && text.GetString() is { Length: > 0 } t)
                    {
                        Hint = t;
                    }
                    break;
                default:
                    return;
            }
            Raise();
        }
        catch
        {
            // 非 JSON 事件忽略.
        }
    }

    private static void ApplyHeadset(JsonElement n)
    {
        var kind = n.GetProperty("kind").GetString();
        switch (kind)
        {
            case "battery":
                Battery = n.GetProperty("percent").GetInt32();
                Hint = $"电量 {Battery}%";
                break;
            case "noise":
                Noise = n.GetProperty("mode").GetString();
                if (n.TryGetProperty("ambient_volume", out var av) && av.TryGetInt32(out var vol))
                {
                    AmbientVolume = vol;
                }
                Hint = "降噪 " + NoiseLabel(Noise);
                break;
            case "name":
                DeviceName = n.GetProperty("name").GetString();
                Hint = "耳机 " + DeviceName;
                break;
            case "mac":
                Mac = n.GetProperty("address").GetString();
                Hint = "MAC " + Mac;
                break;
            case "firmware":
                Firmware = n.GetProperty("version").GetString();
                Hint = "固件 " + Firmware;
                break;
            case "sound_effect":
                Effect = n.GetProperty("effect").GetString();
                Hint = "音效 " + EffectLabel(Effect);
                break;
            case "game_mode":
                GameMode = n.GetProperty("on").GetBoolean();
                Hint = GameMode ? "游戏模式开" : "游戏模式关";
                break;
            case "ldac":
                Ldac = n.GetProperty("mode").GetString();
                Hint = "LDAC " + LdacLabel(Ldac);
                break;
            case "prompt_volume":
                PromptVolume = n.GetProperty("volume").GetInt32();
                Hint = "提示音量 " + PromptVolume;
                break;
            case "shutdown_timer_enabled":
                ShutdownOn = n.GetProperty("on").GetBoolean();
                Hint = ShutdownOn ? "定时关机开" : "定时关机关";
                break;
            case "shutdown_timer":
                ShutdownOn = true;
                ShutdownMinutes = Math.Clamp(n.GetProperty("minutes").GetInt32(), 1, 180);
                Hint = "定时关机 " + ShutdownMinutes + " 分钟";
                break;
            case "auto_power_off":
                AutoPowerOff = n.GetProperty("on").GetBoolean();
                Hint = AutoPowerOff ? "自动关机开" : "自动关机关";
                break;
            case "control_settings":
                ControlNormal = n.GetProperty("normal").GetBoolean();
                ControlReduction = n.GetProperty("reduction").GetBoolean();
                ControlAmbient = n.GetProperty("ambient").GetBoolean();
                Hint = "按键可切换模式已更新";
                break;
        }
    }

    public static string EffectLabel(string? effect) => effect switch
    {
        "normal" => "标准",
        "pop" => "流行",
        "classical" => "古典",
        "rock" => "摇滚",
        _ => effect ?? "未知",
    };

    public static string LdacLabel(string? mode) => mode switch
    {
        "off" => "关闭",
        "rate48k" => "44.1k / 48k",
        "rate96k" => "96k",
        _ => mode ?? "未知",
    };

    private static string HandoffText(JsonElement p)
    {
        return p.GetProperty("kind").GetString() switch
        {
            "requesting" => "正在请求交接",
            "waiting_peer" => "等待对端释放音频",
            "releasing" => "正在释放音频",
            "connecting" => "正在接管音频",
            "fallback_cd" => "对端未释放, 已发 CD",
            "done" => "交接完成",
            "failed" => "交接失败: " + (p.TryGetProperty("reason", out var r) ? r.GetString() : ""),
            "busy" => "交接忙, 请稍后再试",
            _ => Hint,
        };
    }

    private static string AudioText(string? state) => state switch
    {
        "connected" => "系统音频已连接",
        "disconnected" => "系统音频已断开",
        "connecting" => "正在连接系统音频",
        _ => Hint,
    };

    public static void TryAutoJoin()
    {
        if (GroupJoined)
        {
            return;
        }
        try
        {
            var saved = ApplicationData.Current.LocalSettings.Values["passphrase"] as string;
            if (string.IsNullOrWhiteSpace(saved))
            {
                return;
            }
            EdifierNative.EnsureSession();
            EdifierNative.GroupJoin(saved);
            GroupJoined = true;
            Holding = EdifierNative.Holding();
            RefreshPeers();
            SetHint("已自动加入组");
        }
        catch (Exception ex)
        {
            SetHint(ex.Message);
        }
    }

    public static void RefreshPeers()
    {
        if (!GroupJoined)
        {
            return;
        }
        try
        {
            var map = new Dictionary<string, GroupPeer>(StringComparer.Ordinal);
            using var doc = JsonDocument.Parse(EdifierNative.GroupPeers());
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var id = item.GetProperty("id").GetString() ?? "";
                var host = item.TryGetProperty("hostname", out var h) ? h.GetString() ?? id : id;
                if (string.IsNullOrWhiteSpace(host))
                {
                    host = id;
                }
                var holding = item.TryGetProperty("holding", out var hold) && hold.ValueKind == JsonValueKind.String
                    ? hold.GetString()
                    : null;
                map[id] = new GroupPeer(id, host, string.IsNullOrEmpty(holding) ? null : holding);
            }
            var next = map.Values.OrderBy(p => p.Id, StringComparer.Ordinal).ToList();
            if (PeersEqual(Peers, next))
            {
                return;
            }
            Peers = next;
            Raise();
        }
        catch
        {
            // 组员列表暂时失败时保持上一份.
        }
    }

    public static bool SameMac(string? a, string? b)
    {
        if (string.IsNullOrWhiteSpace(a) || string.IsNullOrWhiteSpace(b))
        {
            return false;
        }
        static string Norm(string s) => new string(s.Where(char.IsLetterOrDigit).ToArray());
        return string.Equals(Norm(a), Norm(b), StringComparison.OrdinalIgnoreCase);
    }

    private static bool PeersEqual(IReadOnlyList<GroupPeer> a, IReadOnlyList<GroupPeer> b)
    {
        if (a.Count != b.Count)
        {
            return false;
        }
        for (var i = 0; i < a.Count; i++)
        {
            if (a[i].Id != b[i].Id || a[i].Host != b[i].Host || a[i].Holding != b[i].Holding)
            {
                return false;
            }
        }
        return true;
    }

    public static GroupPeer? HolderOf(string address)
    {
        return Peers.FirstOrDefault(p => SameMac(p.Holding, address));
    }

    public static bool IsEdifierName(string? name)
    {
        if (string.IsNullOrWhiteSpace(name))
        {
            return false;
        }
        return name.Contains("EDIFIER", StringComparison.OrdinalIgnoreCase) || name.Contains("漫步者");
    }

    public static string NoiseLabel(string? mode) => mode switch
    {
        "normal" => "关闭",
        "reduction" => "降噪",
        "ambient" => "通透",
        _ => mode ?? "未知",
    };

    private static void Raise() => Changed?.Invoke();
}

internal sealed record GroupPeer(string Id, string Host, string? Holding);
