using EdifierCtrl.Infrastructure;

namespace EdifierCtrl.Native;

/// <summary>
/// 后台持续读取会话事件, 实际归并由 AppActions 投递到 UI 调度队列.
/// </summary>
internal sealed class EventPump
{
    private readonly Func<int, CancellationToken, Task> _poll;
    private readonly Func<Exception, int, Task> _report;
    private readonly CancellationTokenSource _cancellation = new();
    private Task? _task;

    internal EventPump(Func<int, CancellationToken, Task> poll, Func<Exception, int, Task> report)
    {
        _poll = poll;
        _report = report;
    }

    internal void Start() => _task ??= Task.Run(RunAsync);
    internal void Cancel() => _cancellation.Cancel();
    internal async Task StopAsync()
    {
        Cancel();
        if (_task is not null) await _task.ConfigureAwait(false);
    }

    private async Task RunAsync()
    {
        var token = _cancellation.Token;
        var tick = 0;
        var failures = 0;
        try
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    await _poll(tick, token).ConfigureAwait(false);
                    if (failures > 0) AppLog.Info("状态同步已经恢复.", "session");
                    failures = 0;
                }
                catch (OperationCanceledException) when (token.IsCancellationRequested) { break; }
                catch (Exception error)
                {
                    failures++;
                    await _report(error, failures).ConfigureAwait(false);
                    if (failures >= 3) break;
                }
                tick = (tick + 1) % 8;
                await Task.Delay(failures == 0 ? 250 : 1000, token).ConfigureAwait(false);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception error)
        {
            AppLog.Error($"事件同步任务已经停止: {error}", "session");
            try { await _report(error, 3).ConfigureAwait(false); }
            catch (Exception reportError) { AppLog.Error($"无法报告事件同步错误: {reportError}", "session"); }
        }
    }
}
