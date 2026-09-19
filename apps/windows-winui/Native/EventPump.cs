using Microsoft.UI.Xaml;

namespace EdifierCtrl.Native;

/// <summary>
/// 把 session poll_event 拉到 UI 线程, 写入 SessionState.
/// </summary>
internal sealed class EventPump
{
    private readonly DispatcherTimer _timer;

    public EventPump()
    {
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        _timer.Tick += (_, _) => Drain();
    }

    public void Start() => _timer.Start();

    public void Stop() => _timer.Stop();

    private static void Drain()
    {
        try
        {
            for (var i = 0; i < 32; i++)
            {
                var ev = EdifierNative.PollEvent();
                if (ev.Contains("\"empty\""))
                {
                    break;
                }
                SessionState.ApplyEvent(ev);
            }
        }
        catch
        {
            // 没有会话时保持安静.
        }
    }
}
