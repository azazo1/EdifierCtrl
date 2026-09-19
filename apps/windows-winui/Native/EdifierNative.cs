using System.Runtime.InteropServices;

namespace EdifierCtrl.Native;

/// <summary>
/// P/Invoke 对应 crates/edifier-ffi/include/edifier.h.
/// </summary>
public static class EdifierNative
{
    private const string Dll = "edifier_ffi";

    public static IntPtr Session { get; private set; }

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_version();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_last_error();

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern void edifier_string_free(IntPtr s);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_command_encode(byte[] commandJson);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_frame_parse(byte[] frameHex);

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
    private static extern int edifier_session_readout(IntPtr session, byte[] profileKey);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_send_json(IntPtr session, byte[] commandJson);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_poll_event(IntPtr session);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_join(IntPtr session, byte[] passphrase);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_group_peers(IntPtr session);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_claim(IntPtr session, byte[] mac);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_group_claim_peer(IntPtr session, byte[] peerId);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_group_id_hex(IntPtr session);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int edifier_session_set_holding(IntPtr session, byte[] mac);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr edifier_session_holding(IntPtr session);

    public static string Version() => ReadConst(edifier_version());

    public static string ProfilesJson() => Take(edifier_profiles_json());

    public static string EncodeCommand(string json) => Take(edifier_command_encode(Utf8Z(json)));

    public static string ParseFrame(string hex) => Take(edifier_frame_parse(Utf8Z(hex)));

    public static void EnsureSession()
    {
        if (Session != IntPtr.Zero)
        {
            return;
        }
        Session = edifier_session_new(Utf8Z(Environment.MachineName + "-" + Environment.ProcessId));
        if (Session == IntPtr.Zero)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void Shutdown()
    {
        if (Session == IntPtr.Zero)
        {
            return;
        }
        edifier_session_free(Session);
        Session = IntPtr.Zero;
    }

    public static string Scan(string kind) => Take(edifier_session_scan(NeedSession(), Utf8Z(kind)));

    public static void Connect(string address, string kind)
    {
        if (edifier_session_connect(NeedSession(), Utf8Z(address), Utf8Z(kind)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void Disconnect()
    {
        if (edifier_session_disconnect(NeedSession()) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void Readout(string profileKey)
    {
        if (edifier_session_readout(NeedSession(), Utf8Z(profileKey)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void SendJson(string json)
    {
        if (edifier_session_send_json(NeedSession(), Utf8Z(json)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static string PollEvent() => Take(edifier_session_poll_event(NeedSession()));

    public static void GroupJoin(string passphrase)
    {
        if (edifier_session_group_join(NeedSession(), Utf8Z(passphrase)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static string GroupPeers() => Take(edifier_session_group_peers(NeedSession()));

    public static string GroupIdHex() => Take(edifier_session_group_id_hex(NeedSession()));

    public static void GroupClaim(string mac)
    {
        if (edifier_session_group_claim(NeedSession(), Utf8Z(mac)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void GroupClaimPeer(string peerId)
    {
        if (edifier_session_group_claim_peer(NeedSession(), Utf8Z(peerId)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static void SetHolding(string mac)
    {
        if (edifier_session_set_holding(NeedSession(), Utf8Z(mac)) != 0)
        {
            throw new InvalidOperationException(LastError());
        }
    }

    public static string Holding() => Take(edifier_session_holding(NeedSession()));

    private static IntPtr NeedSession()
    {
        EnsureSession();
        return Session;
    }

    private static string LastError() => Marshal.PtrToStringUTF8(edifier_last_error()) ?? "未知 FFI 错误";

    private static byte[] Utf8Z(string s)
    {
        var bytes = System.Text.Encoding.UTF8.GetBytes(s);
        var withNul = new byte[bytes.Length + 1];
        Buffer.BlockCopy(bytes, 0, withNul, 0, bytes.Length);
        return withNul;
    }

    private static string ReadConst(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException("edifier_version 为空");
        }
        return Marshal.PtrToStringUTF8(ptr) ?? "";
    }

    private static string Take(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException(LastError());
        }
        try
        {
            return Marshal.PtrToStringUTF8(ptr) ?? "";
        }
        finally
        {
            edifier_string_free(ptr);
        }
    }
}
