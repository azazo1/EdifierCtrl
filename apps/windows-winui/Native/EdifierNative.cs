using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using EdifierCtrl.Infrastructure;

namespace EdifierCtrl.Native;

/// <summary>
/// 所有 C ABI 调用都在后台同一把锁内完成, 包括错误读取, 字符串释放和会话销毁.
/// </summary>
internal static class EdifierNative
{
    private const string Dll = "edifier_ffi";
    private static readonly SemaphoreSlim Gate = new(1, 1);
    private static readonly LogCallback NativeLog = ForwardLog;
    private static IntPtr _session;
    private static bool _stopped;
    private static bool _loggingInstalled;
    private static bool _groupJoined;
    private static int _logLevel;

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void LogCallback(int level, IntPtr target, IntPtr message, int flush);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_log_install(LogCallback callback);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_log_set_level(int level);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_version();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_last_error();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern void edifier_string_free(IntPtr value);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_command_encode(byte[] json);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_frame_parse(byte[] hex);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_profiles_json();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_new(byte[] localId);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern void edifier_session_free(IntPtr session);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_scan(IntPtr session, byte[] kind);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_connect(IntPtr session, byte[] address, byte[] kind);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_disconnect(IntPtr session);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_readout(IntPtr session, byte[] profile);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_send_json(IntPtr session, byte[] json);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_poll_event(IntPtr session);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_join(IntPtr session, byte[] group);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_leave(IntPtr session);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_group_peers(IntPtr session);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_claim(IntPtr session, byte[] mac);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_claim_peer(IntPtr session, byte[] peerId);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_holding(IntPtr session);

    internal static Task<(string Version, IReadOnlyList<HeadphoneProfile> Profiles)> PrepareAsync(string localId) => PerformAsync(() =>
    {
        if (!_loggingInstalled)
        {
            Check(edifier_log_install(NativeLog));
            _loggingInstalled = true;
        }
        UpdateLogLevel();
        if (_session == IntPtr.Zero)
        {
            _session = edifier_session_new(Utf8Z(localId));
            if (_session == IntPtr.Zero) throw Failure();
        }
        var version = Marshal.PtrToStringUTF8(edifier_version()) ?? throw new InvalidOperationException("核心版本为空.");
        var profiles = NativeJson.Decode<List<HeadphoneProfile>>(Take(edifier_profiles_json()));
        if (profiles.Count == 0 || profiles.Any(p => string.IsNullOrWhiteSpace(p.Id) || p.Capabilities is null || p.MaxNameLen <= 0))
            throw new JsonException("核心机型档案缺少有效的功能列表或名称长度.");
        return (version, (IReadOnlyList<HeadphoneProfile>)profiles);
    });

    internal static Task<IReadOnlyList<HeadphoneDevice>> ScanAsync(string kind) => PerformAsync<IReadOnlyList<HeadphoneDevice>>(() =>
        NativeJson.Decode<List<HeadphoneDevice>>(Take(edifier_session_scan(NeedSession(), Utf8Z(kind)))));

    internal static Task ConnectAsync(string address, string kind) => PerformAsync(() =>
        Check(edifier_session_connect(NeedSession(), Utf8Z(address), Utf8Z(kind))));
    internal static Task DisconnectAsync() => PerformAsync(() => Check(edifier_session_disconnect(NeedSession())));
    internal static Task ReadoutAsync(string profile) => PerformAsync(() => Check(edifier_session_readout(NeedSession(), Utf8Z(profile))));
    internal static Task SendAsync(string json) => PerformAsync(() => Check(edifier_session_send_json(NeedSession(), Utf8Z(json))));
    internal static Task JoinAsync(string group) => PerformAsync(() =>
    {
        Check(edifier_session_group_join(NeedSession(), Utf8Z(group)));
        _groupJoined = true;
    });
    internal static Task LeaveAsync() => PerformAsync(() =>
    {
        Check(edifier_session_group_leave(NeedSession()));
        _groupJoined = false;
    });
    internal static Task ClaimAsync(string mac) => PerformAsync(() => Check(edifier_session_group_claim(NeedSession(), Utf8Z(mac))));
    internal static Task ClaimPeerAsync(string id) => PerformAsync(() => Check(edifier_session_group_claim_peer(NeedSession(), Utf8Z(id))));

    internal static Task<NativeSnapshot> PollAsync(bool includePeers, CancellationToken cancellationToken) => PerformAsync(() =>
    {
        var events = new List<JsonElement>();
        for (var index = 0; index < 64; index++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            using var json = JsonDocument.Parse(Take(edifier_session_poll_event(NeedSession())));
            var kind = json.RootElement.GetProperty("kind").GetString();
            if (kind == "empty") break;
            events.Add(json.RootElement.Clone());
        }
        IReadOnlyList<GroupPeer>? peers = null;
        if (_groupJoined && includePeers)
            peers = NativeJson.Decode<List<GroupPeer>>(Take(edifier_session_group_peers(NeedSession())));
        var holding = Take(edifier_session_holding(NeedSession()));
        return new NativeSnapshot(events, peers, string.IsNullOrWhiteSpace(holding) ? null : AppActions.NormalizeAddress(holding));
    }, cancellationToken);

    internal static Task<string> DiagnosticAsync(string input, bool parse) => PerformAsync(() =>
    {
        var raw = Take(parse ? edifier_frame_parse(Utf8Z(input)) : edifier_command_encode(Utf8Z(input)));
        using var json = JsonDocument.Parse(raw);
        return JsonSerializer.Serialize(json.RootElement, new JsonSerializerOptions { WriteIndented = true });
    });

    internal static async Task StopAsync()
    {
        await Gate.WaitAsync().ConfigureAwait(false);
        try
        {
            await Task.Run(() =>
            {
                _stopped = true;
                var session = _session;
                _session = IntPtr.Zero;
                _groupJoined = false;
                if (session != IntPtr.Zero) edifier_session_free(session);
            }).ConfigureAwait(false);
        }
        finally { Gate.Release(); }
    }

    private static async Task<T> PerformAsync<T>(Func<T> action, CancellationToken cancellationToken = default)
    {
        await Gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            return await Task.Run(() =>
            {
                cancellationToken.ThrowIfCancellationRequested();
                if (_stopped) throw new InvalidOperationException("耳机服务已经停止.");
                if (_loggingInstalled) UpdateLogLevel();
                return action();
            }, cancellationToken).ConfigureAwait(false);
        }
        finally { Gate.Release(); }
    }

    private static Task PerformAsync(Action action) => PerformAsync(() => { action(); return true; });

    private static void UpdateLogLevel()
    {
        var level = AppLog.NativeLevel;
        if (_logLevel == level) return;
        Check(edifier_log_set_level(level));
        _logLevel = level;
    }

    private static void ForwardLog(int level, IntPtr target, IntPtr message, int flush)
    {
        try
        {
            AppLog.Native(level, Marshal.PtrToStringUTF8(target) ?? "native", Marshal.PtrToStringUTF8(message) ?? "", flush != 0);
        }
        catch
        {
            // C ABI 日志回调不能让托管异常越过原生边界.
        }
    }

    private static IntPtr NeedSession() => _session != IntPtr.Zero ? _session : throw new InvalidOperationException("耳机服务尚未启动.");
    private static Exception Failure() => new InvalidOperationException(Marshal.PtrToStringUTF8(edifier_last_error()) ?? "耳机服务未返回详细错误.");
    private static void Check(int result) { if (result != 0) throw Failure(); }
    private static byte[] Utf8Z(string value)
    {
        if (value.Contains('\0')) throw new ArgumentException("输入不能含有空字符.");
        return Encoding.UTF8.GetBytes(value + "\0");
    }
    private static string Take(IntPtr pointer)
    {
        if (pointer == IntPtr.Zero) throw Failure();
        try { return Marshal.PtrToStringUTF8(pointer) ?? ""; }
        finally { edifier_string_free(pointer); }
    }
}
