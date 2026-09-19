using Microsoft.UI.Xaml;

namespace EdifierCtrl.Native;

/// <summary>
/// 把 session poll_event 拉到 UI 线程.
/// </summary>
internal sealed class EventPump
{
    private readonly DispatcherTimer _timer;
    private readonly Action<string> _onEvent;

    public EventPump(Action<string> onEvent)
    {
        _onEvent = onEvent;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(250) };
        _timer.Tick += (_, _) => Drain();
    }

    public void Start() => _timer.Start();

    public void Stop() => _timer.Stop();

    private void Drain()
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
                _onEvent(ev);
            }
        }
        catch
        {
            // 没有会话时保持安静.
        }
    }
}
