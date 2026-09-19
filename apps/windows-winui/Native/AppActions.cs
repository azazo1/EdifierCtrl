using System.Text;
using System.Text.Json;
using EdifierCtrl.Infrastructure;
using Microsoft.UI.Dispatching;

namespace EdifierCtrl.Native;

/// <summary>
/// UI 的唯一操作入口. 操作与轮询归并串行执行, 原生阻塞调用始终在后台.
/// </summary>
public static class AppActions
{
    private static readonly SemaphoreSlim Operations = new(1, 1);
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private static DispatcherQueue? _dispatcher;
    private static EventPump? _pump;
    private static bool _started;
    private static int _stopping;
    private static DateTimeOffset? _handoffStarted;
    private static string? _claimTarget;

    public static async Task StartAsync(DispatcherQueue dispatcher)
    {
        try
        {
            await Operations.WaitAsync().ConfigureAwait(false);
            try
            {
                if (_started || Volatile.Read(ref _stopping) != 0) return;
                _started = true;
                _dispatcher = dispatcher;
                SessionState.Attach(dispatcher);
                await OnUiAsync(() => SessionState.SetOperation("正在启动耳机服务")).ConfigureAwait(false);
                var preferences = await OnUiAsync(() =>
                {
                    var prefs = AppPreferences.Current;
                    return (prefs.InstallationId, prefs.SelectedProfile, prefs.LastDeviceAddress, prefs.LastDeviceName);
                }).ConfigureAwait(false);
                var result = await EdifierNative.PrepareAsync($"{Environment.MachineName}-{preferences.InstallationId}").ConfigureAwait(false);
                if (Volatile.Read(ref _stopping) != 0) return;
                await OnUiAsync(() =>
                {
                    SessionState.Initialize(result.Version, result.Profiles, preferences.SelectedProfile, preferences.LastDeviceAddress, preferences.LastDeviceName);
                    if (AppPreferences.Current.SelectedProfile != SessionState.SelectedProfile.Id)
                    {
                        AppPreferences.Current.SelectedProfile = SessionState.SelectedProfile.Id;
                        SavePreferences("机型已选择, 但无法保存设置");
                    }
                }).ConfigureAwait(false);
                _pump = new EventPump(PollAsync, PollFailedAsync);
                _pump.Start();
            }
            catch (Exception error)
            {
                await OnUiAsync(() => SessionState.SetReady(false)).ConfigureAwait(false);
                await ReportAsync("耳机服务启动失败", error).ConfigureAwait(false);
            }
            finally
            {
                try { await OnUiAsync(() => SessionState.SetOperation(null)).ConfigureAwait(false); }
                finally { Operations.Release(); }
            }
            var group = await OnUiAsync(() =>
            {
                var prefs = AppPreferences.Current;
                return prefs.RememberGroup && prefs.AutoJoinGroup ? prefs.GroupName : "";
            }).ConfigureAwait(false);
            if (!string.IsNullOrWhiteSpace(group) && Volatile.Read(ref _stopping) == 0
                && await OnUiAsync(() => SessionState.Ready).ConfigureAwait(false))
                await JoinAsync(group, true).ConfigureAwait(false);
        }
        catch (Exception error) { await ReportAsync("耳机服务启动失败", error).ConfigureAwait(false); }
    }

    public static async Task StopAsync()
    {
        Interlocked.Exchange(ref _stopping, 1);
        try
        {
            _pump?.Cancel();
            if (_pump is not null) await _pump.StopAsync().ConfigureAwait(false);
            await Operations.WaitAsync().ConfigureAwait(false);
            try
            {
                // 启动可能在第一次取消之后才创建事件泵, 此时操作门已确保启动完成.
                _pump?.Cancel();
                await EdifierNative.StopAsync().ConfigureAwait(false);
                _claimTarget = null;
                _handoffStarted = null;
                await OnUiAsync(SessionState.Stop).ConfigureAwait(false);
            }
            finally { Operations.Release(); }
            if (_pump is not null) await _pump.StopAsync().ConfigureAwait(false);
        }
        catch (Exception error) { await ReportAsync("耳机服务停止失败", error).ConfigureAwait(false); }
        finally { AppLog.Flush(); }
    }

    public static Task ScanAsync(string kind = "rfcomm") => RunAsync("正在查找已配对的耳机", async () =>
    {
        ValidateKind(kind);
        var devices = await EdifierNative.ScanAsync(kind).ConfigureAwait(false);
        await OnUiAsync(() =>
        {
            SessionState.SetDevices(devices);
            SessionState.Record("设备列表已更新", $"发现 {devices.Count} 台已配对设备");
        }).ConfigureAwait(false);
    });

    public static Task ConnectAsync(string address, string name = "", string kind = "rfcomm") => RunAsync("正在连接耳机", async () =>
    {
        var normalized = NormalizeAddress(address);
        ValidateKind(kind);
        await EdifierNative.ConnectAsync(normalized, kind).ConfigureAwait(false);
        var profile = await OnUiAsync(() =>
        {
            SessionState.SetConnected(true, normalized, name);
            var prefs = AppPreferences.Current;
            prefs.LastDeviceAddress = normalized;
            prefs.LastDeviceName = name;
            var compactName = name.Replace(" ", "", StringComparison.Ordinal);
            var device = SessionState.Devices.FirstOrDefault(item => SessionState.SameMac(item.Address, normalized));
            var detected = SessionState.Profiles.FirstOrDefault(item => item.ServiceUuid is not null
                && string.Equals(item.ServiceUuid, device?.ServiceUuid, StringComparison.OrdinalIgnoreCase))
                ?? SessionState.Profiles.OrderByDescending(item => item.Id.Length).FirstOrDefault(item => item.Id != "basedevice"
                    && compactName.Contains(item.Id, StringComparison.OrdinalIgnoreCase));
            if (detected is not null) SessionState.SelectProfile(detected);
            prefs.SelectedProfile = SessionState.SelectedProfile.Id;
            SessionState.Notify("控制通道已连接", "正在读取耳机状态. 音频连接结果会单独显示.");
            SavePreferences("已连接, 但无法保存设备设置");
            return SessionState.SelectedProfile.Id;
        }).ConfigureAwait(false);
        await EdifierNative.ReadoutAsync(profile).ConfigureAwait(false);
    });

    public static Task DisconnectAsync() => RunAsync("正在断开控制通道", async () =>
    {
        await EdifierNative.DisconnectAsync().ConfigureAwait(false);
        await OnUiAsync(() =>
        {
            SessionState.SetConnected(false);
            SessionState.Notify("控制通道已断开", "系统音频保持当前连接. 如需转移声音, 请使用跨设备交接.");
        }).ConfigureAwait(false);
    });

    public static Task ReadoutAsync() => RunAsync("正在读取耳机状态", async () =>
    {
        var profile = await OnUiAsync(() => SessionState.SelectedProfile.Id).ConfigureAwait(false);
        await EdifierNative.ReadoutAsync(profile).ConfigureAwait(false);
        await OnUiAsync(() => SessionState.Record("已请求读取状态", "读数以耳机回报为准.")).ConfigureAwait(false);
    }, requireConnection: true);

    public static Task SelectProfileAsync(string id) => RunAsync("正在切换机型", async () =>
    {
        var connected = await OnUiAsync(() =>
        {
            var profile = SessionState.Profiles.FirstOrDefault(item => item.Id == id)
                ?? throw new ArgumentException("机型档案不存在.");
            SessionState.SelectProfile(profile);
            AppPreferences.Current.SelectedProfile = profile.Id;
            SavePreferences("机型已切换, 但无法保存设置");
            return SessionState.Connected;
        }).ConfigureAwait(false);
        if (connected) await EdifierNative.ReadoutAsync(id).ConfigureAwait(false);
    });

    public static Task SendAsync(string op, object? values = null, string? label = null, string? query = null) =>
        RunAsync(label ?? "正在发送指令", async () =>
        {
            var command = BuildCommand(op, values);
            await OnUiAsync(() => ValidateCommand(op, command)).ConfigureAwait(false);
            await EdifierNative.SendAsync(JsonSerializer.Serialize(command)).ConfigureAwait(false);
            await OnUiAsync(() => SessionState.Notify("指令已发送", $"{label ?? op}. 状态以耳机回报为准.")).ConfigureAwait(false);
            if (!string.IsNullOrWhiteSpace(query))
            {
                var followup = BuildCommand(query, null);
                await OnUiAsync(() => ValidateCommand(query, followup)).ConfigureAwait(false);
                await EdifierNative.SendAsync(JsonSerializer.Serialize(followup)).ConfigureAwait(false);
            }
        }, requireConnection: true);

    public static Task JoinAsync(string group, bool remember) => RunAsync("正在加入交接组", async () =>
    {
        if (string.IsNullOrWhiteSpace(group)) throw new ArgumentException("请输入组名, 并在各设备上使用完全相同的内容.");
        if (await OnUiAsync(() => SessionState.GroupJoined).ConfigureAwait(false))
            throw new InvalidOperationException("已加入交接组, 请先退出当前组.");
        await EdifierNative.JoinAsync(group).ConfigureAwait(false);
        await OnUiAsync(() =>
        {
            SessionState.SetGroupJoined(true, group);
            SessionState.Notify("已加入交接组", "同一网络中使用相同组名的设备会自动出现在这里.");
            var prefs = AppPreferences.Current;
            prefs.RememberGroup = remember;
            prefs.GroupName = remember ? group : "";
            if (!remember) prefs.AutoJoinGroup = false;
            SavePreferences("已入组, 但无法保存组设置");
        }).ConfigureAwait(false);
        await ConsumeAsync(await EdifierNative.PollAsync(true, CancellationToken.None).ConfigureAwait(false)).ConfigureAwait(false);
    });

    public static Task LeaveAsync() => RunAsync("正在退出交接组", async () =>
    {
        await EdifierNative.LeaveAsync().ConfigureAwait(false);
        await OnUiAsync(() =>
        {
            _claimTarget = null;
            _handoffStarted = null;
            SessionState.SetGroupJoined(false);
            SessionState.Notify("已退出交接组", "本机耳机控制仍可继续使用.");
        }).ConfigureAwait(false);
    }, allowDuringHandoff: true);

    public static Task ClaimPeerAsync(string id) => RunAsync("正在请求接管", async () =>
    {
        var peer = await OnUiAsync(() =>
        {
            RequireGroup();
            return SessionState.Peers.FirstOrDefault(item => item.Id == id) ?? throw new ArgumentException("该组成员已经离线.");
        }).ConfigureAwait(false);
        if (!peer.CanAudio || string.IsNullOrWhiteSpace(peer.Holding)) throw new InvalidOperationException("该成员没有可交接的系统音频连接.");
        await BeginClaimAsync(NormalizeAddress(peer.Holding), peer.DisplayName, () => EdifierNative.ClaimPeerAsync(id)).ConfigureAwait(false);
    });

    public static Task ClaimAsync(string mac) => RunAsync("正在请求接管", async () =>
    {
        var normalized = NormalizeAddress(mac);
        await OnUiAsync(RequireGroup).ConfigureAwait(false);
        await BeginClaimAsync(normalized, normalized, () => EdifierNative.ClaimAsync(normalized)).ConfigureAwait(false);
    });

    public static async Task<string> DiagnosticAsync(string input, bool parse)
    {
        var output = "";
        await RunAsync(parse ? "正在解析耳机数据" : "正在编码指令", async () =>
        {
            output = await EdifierNative.DiagnosticAsync(input, parse).ConfigureAwait(false);
        }).ConfigureAwait(false);
        return output;
    }

    public static string NormalizeAddress(string address) => TryNormalizeAddress(address)
        ?? throw new ArgumentException("蓝牙地址必须为 12 位十六进制数字, 例如 AA:BB:CC:DD:EE:FF.");

    internal static string? TryNormalizeAddress(string? address)
    {
        if (string.IsNullOrWhiteSpace(address)) return null;
        var compact = address.Trim().Replace(":", "", StringComparison.Ordinal).Replace("-", "", StringComparison.Ordinal);
        if (compact.Length != 12 || compact.Any(character => !((character >= '0' && character <= '9')
            || (character >= 'A' && character <= 'F') || (character >= 'a' && character <= 'f')))) return null;
        compact = compact.ToUpperInvariant();
        return string.Join(":", Enumerable.Range(0, 6).Select(index => compact.Substring(index * 2, 2)));
    }

    private static async Task BeginClaimAsync(string address, string label, Func<Task> action)
    {
        await OnUiAsync(() =>
        {
            if (SessionState.SameMac(SessionState.Holding, address)) throw new InvalidOperationException("本机已经持有这副耳机的系统音频.");
            _claimTarget = address;
            _handoffStarted = DateTimeOffset.UtcNow;
            SessionState.SetHandoff("requesting");
        }).ConfigureAwait(false);
        try
        {
            await action().ConfigureAwait(false);
            await OnUiAsync(() => SessionState.Record("请求接管耳机", label)).ConfigureAwait(false);
        }
        catch (Exception error)
        {
            await OnUiAsync(() =>
            {
                _claimTarget = null;
                _handoffStarted = null;
                SessionState.SetHandoff("failed", error.Message);
            }).ConfigureAwait(false);
            throw;
        }
    }

    private static async Task RunAsync(string label, Func<Task> action, bool requireConnection = false, bool allowDuringHandoff = false)
    {
        var entered = false;
        var operationStarted = false;
        try
        {
            await Operations.WaitAsync().ConfigureAwait(false);
            entered = true;
            if (Volatile.Read(ref _stopping) != 0) return;
            await OnUiAsync(() =>
            {
                if (!SessionState.Ready) throw new InvalidOperationException("耳机服务未就绪, 请查看启动或同步错误.");
                if (SessionState.HandoffActive && !allowDuringHandoff) throw new InvalidOperationException("交接仍在进行, 请等待完成或退出交接组.");
                if (requireConnection && !SessionState.Connected) throw new InvalidOperationException("请先连接耳机控制通道.");
                SessionState.SetOperation(label);
                operationStarted = true;
                SessionState.Record(label);
            }).ConfigureAwait(false);
            await action().ConfigureAwait(false);
        }
        catch (Exception error)
        {
            if (Volatile.Read(ref _stopping) == 0) await ReportAsync(label + "失败", error).ConfigureAwait(false);
            else AppLog.Warn($"退出时操作未完成: {error.Message}", "session");
        }
        finally
        {
            if (entered)
            {
                try
                {
                    if (operationStarted) await OnUiAsync(() => SessionState.SetOperation(null)).ConfigureAwait(false);
                }
                catch (Exception error) { AppLog.Error($"操作状态清理失败: {error.Message}", "session"); }
                finally { Operations.Release(); }
            }
        }
    }

    private static async Task PollAsync(int tick, CancellationToken cancellationToken)
    {
        await Operations.WaitAsync(cancellationToken).ConfigureAwait(false);
        string? reconnect = null;
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (Volatile.Read(ref _stopping) != 0) return;
            var snapshot = await EdifierNative.PollAsync(tick % 8 == 0, cancellationToken).ConfigureAwait(false);
            cancellationToken.ThrowIfCancellationRequested();
            reconnect = await ConsumeAsync(snapshot).ConfigureAwait(false);
        }
        finally { Operations.Release(); }
        if (reconnect is not null && Volatile.Read(ref _stopping) == 0) await ConnectAsync(reconnect).ConfigureAwait(false);
    }

    private static Task<string?> ConsumeAsync(NativeSnapshot snapshot) => OnUiAsync(() =>
    {
        SessionState.ApplySnapshot(snapshot);
        if (SessionState.HandoffActive)
        {
            _handoffStarted ??= DateTimeOffset.UtcNow;
            if (DateTimeOffset.UtcNow - _handoffStarted.Value > TimeSpan.FromSeconds(35))
            {
                const string reason = "未能在预期时间内确认音频连接. 请检查两端蓝牙状态后重试.";
                SessionState.SetHandoff("failed", reason);
                SessionState.Notify("交接超时", reason, true);
                _claimTarget = null;
                _handoffStarted = null;
            }
        }
        else
        {
            _handoffStarted = null;
            if (!SessionState.HandoffDone) _claimTarget = null;
        }
        if (SessionState.HandoffDone && _claimTarget is { } target && SessionState.SameMac(SessionState.Holding, target) && !SessionState.Busy)
        {
            _claimTarget = null;
            if (!SessionState.Connected || !SessionState.SameMac(SessionState.Address, target)) return target;
        }
        return (string?)null;
    });

    private static async Task PollFailedAsync(Exception error, int failures)
    {
        if (Volatile.Read(ref _stopping) != 0) return;
        AppLog.Error($"状态轮询失败 ({failures}/3): {error}", "session");
        await OnUiAsync(() =>
        {
            if (failures >= 3)
            {
                SessionState.SetReady(false);
                SessionState.Notify("状态同步已停止", $"连续 3 次同步失败. 请重新启动应用. {error.Message}", true);
            }
            else if (failures == 1) SessionState.Notify("状态同步失败", error.Message, true);
        }).ConfigureAwait(false);
    }

    private static Dictionary<string, JsonElement> BuildCommand(string op, object? values)
    {
        if (string.IsNullOrWhiteSpace(op)) throw new ArgumentException("指令名称不能为空.");
        var command = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        if (values is not null)
        {
            var element = JsonSerializer.SerializeToElement(values);
            if (element.ValueKind != JsonValueKind.Object) throw new ArgumentException("指令参数必须为对象.");
            foreach (var property in element.EnumerateObject()) command[property.Name] = property.Value.Clone();
        }
        command["op"] = JsonSerializer.SerializeToElement(op);
        return command;
    }

    private static void ValidateCommand(string op, IReadOnlyDictionary<string, JsonElement> command)
    {
        var capability = op switch
        {
            "set_noise_mode" or "query_noise" => "noise",
            "set_ambient_volume" => "ambient_sound",
            "set_sound_effect" or "query_sound_effect" => "sound_effect",
            "set_control_settings" or "query_control_settings" => "control_settings",
            "set_ldac" or "query_ldac" => "ldac",
            "set_game_mode" or "query_game_mode" => "game_mode",
            "set_auto_power_off" or "query_auto_power_off" => "auto_power_off",
            "set_prompt_volume" or "query_prompt_volume" => "prompt_volume",
            "set_shutdown_timer" or "disable_shutdown_timer" or "query_shutdown_timer" => "shutdown_timer",
            "set_name" or "query_name" => "name",
            "playback" or "query_playback" => "playback",
            _ => null,
        };
        if (capability is not null && !SessionState.Supports(capability)) throw new InvalidOperationException("当前机型不支持这项功能.");
        if (op == "set_name")
        {
            if (!command.TryGetValue("name", out var name) || name.ValueKind != JsonValueKind.String) throw new ArgumentException("请输入有效的耳机名称.");
            if (StrictUtf8.GetByteCount(name.GetString() ?? "") > SessionState.SelectedProfile.MaxNameLen)
                throw new ArgumentException($"当前机型名称最多支持 {SessionState.SelectedProfile.MaxNameLen} 个 UTF-8 字节.");
        }
        if (op == "set_control_settings" && new[] { "normal", "reduction", "ambient" }
            .Count(key => command.TryGetValue(key, out var value) && value.ValueKind == JsonValueKind.True) < 2)
            throw new ArgumentException("请选择至少两种模式, 耳机按键需要循环切换.");
    }

    private static void ValidateKind(string kind)
    {
        if (kind is not ("rfcomm" or "ble")) throw new ArgumentException("连接类型必须为 rfcomm 或 ble.");
    }
    private static void RequireGroup()
    {
        if (!SessionState.GroupJoined) throw new InvalidOperationException("请先加入交接组.");
    }
    private static void SavePreferences(string failureTitle)
    {
        try
        {
            AppPreferences.Current.Save();
            if (AppPreferences.Current.SaveError is { Length: > 0 } error) SessionState.Notify(failureTitle, error, true);
        }
        catch (Exception error) { SessionState.Notify(failureTitle, error.Message, true); }
    }
    private static async Task ReportAsync(string title, Exception error)
    {
        AppLog.Error($"{title}: {error}", "session");
        try { await OnUiAsync(() => SessionState.Notify(title, error.Message, true)).ConfigureAwait(false); }
        catch (Exception dispatchError) { AppLog.Error($"无法显示操作错误: {dispatchError.Message}", "ui"); }
    }
    private static Task OnUiAsync(Action action) => OnUiAsync(() => { action(); return true; });
    private static Task<T> OnUiAsync<T>(Func<T> action)
    {
        var dispatcher = _dispatcher;
        if (dispatcher is null) return Task.FromException<T>(new InvalidOperationException("UI 调度队列尚未初始化."));
        if (dispatcher.HasThreadAccess)
        {
            try { return Task.FromResult(action()); }
            catch (Exception error) { return Task.FromException<T>(error); }
        }
        var completion = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (!dispatcher.TryEnqueue(() =>
        {
            try { completion.TrySetResult(action()); }
            catch (Exception error) { completion.TrySetException(error); }
        })) completion.TrySetException(new InvalidOperationException("窗口正在退出, 无法更新界面."));
        return completion.Task;
    }
}
