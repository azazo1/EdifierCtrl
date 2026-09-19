using System.IO.Pipes;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using EdifierCtrl.Infrastructure;

namespace EdifierCtrl.Desktop;

internal sealed class SingleInstanceController : IDisposable
{
    private readonly CancellationTokenSource _stop = new();
    private Mutex? _mutex;
    private bool _owned;
    private Task? _listener;
    private string _name = "";

    public bool Acquire()
    {
        Directory.CreateDirectory(AppPaths.DataDirectory);
        var identity = Path.TrimEndingDirectorySeparator(AppPaths.DataDirectory).ToUpperInvariant();
        _name = "EdifierCtrl-" + Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(identity)))[..24];
        _mutex = new Mutex(false, "Local\\" + _name);
        try { _owned = _mutex.WaitOne(0); }
        catch (AbandonedMutexException) { _owned = true; }
        return _owned;
    }

    public async Task ForwardAsync(string arguments)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        using var pipe = new NamedPipeClientStream(".", _name, PipeDirection.Out, PipeOptions.Asynchronous);
        await pipe.ConnectAsync(timeout.Token);
        using var writer = new StreamWriter(pipe, new UTF8Encoding(false));
        await writer.WriteLineAsync(JsonSerializer.Serialize(arguments).AsMemory(), timeout.Token);
        await writer.FlushAsync(timeout.Token);
    }

    public void Listen(Action<string> onActivation)
    {
        _listener = Task.Run(async () =>
        {
            while (!_stop.IsCancellationRequested)
            {
                try
                {
                    using var pipe = new NamedPipeServerStream(_name, PipeDirection.In, 1, PipeTransmissionMode.Byte,
                        PipeOptions.Asynchronous | PipeOptions.CurrentUserOnly);
                    await pipe.WaitForConnectionAsync(_stop.Token);
                    using var reader = new StreamReader(pipe, Encoding.UTF8);
                    using var deadline = CancellationTokenSource.CreateLinkedTokenSource(_stop.Token);
                    deadline.CancelAfter(TimeSpan.FromSeconds(3));
                    var line = await reader.ReadLineAsync(deadline.Token);
                    if (line is not null && line.Length <= 32768)
                        onActivation(JsonSerializer.Deserialize<string>(line) ?? "");
                }
                catch (OperationCanceledException) when (_stop.IsCancellationRequested) { break; }
                catch (Exception ex) { AppLog.Warn("二次启动参数接收失败: " + ex.Message, "desktop"); }
            }
        });
    }

    public async Task StopAsync()
    {
        _stop.Cancel();
        if (_listener is not null) await _listener;
    }

    public void Dispose()
    {
        _stop.Cancel();
        if (_owned) { _mutex?.ReleaseMutex(); _owned = false; }
        _mutex?.Dispose();
        _stop.Dispose();
    }
}
