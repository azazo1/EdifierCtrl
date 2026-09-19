using System.Diagnostics;
using System.Globalization;

namespace EdifierCtrl.Infrastructure;

internal static class AppLog
{
    private static readonly TraceSource Source = CreateSource();
    private static volatile bool _verbose;
    private static readonly string? ForcedLevel = Environment.GetEnvironmentVariable("EDIFIER_LOG_LEVEL")?.ToLowerInvariant();
    public static int NativeLevel => _verbose || ForcedLevel == "trace" ? 5 : ForcedLevel == "debug" ? 4 : 3;

    private static TraceSource CreateSource()
    {
        var source = new TraceSource("EdifierCtrl", SourceLevels.All);
        source.Listeners.Clear();
        source.Listeners.Add(new RotatingLogListener());
        return source;
    }

    public static void SetVerbose(bool enabled) => _verbose = enabled;
    public static void Info(string message, string category = "app") => Record(3, category, message);
    public static void Warn(string message, string category = "app") => Record(2, category, message);
    public static void Error(string message, string category = "app") => Record(1, category, message);
    public static void Debug(string message, string category = "app") => Record(4, category, message);
    public static void Native(int level, string target, string message, bool flush)
    {
        // 原生日志已按 Rust 的 target 指令过滤, 保留其显式覆盖.
        Record(level, target, message, filtered: true);
        if (flush) Flush();
    }
    public static void Flush() => Source.Flush();

    private static void Record(int level, string category, string message, bool filtered = false)
    {
        if (!filtered && level > NativeLevel) return;
        var label = level switch { 1 => "ERROR", 2 => "WARN", 4 => "DEBUG", 5 => "TRACE", _ => "INFO" };
        var eventType = level switch { 1 => TraceEventType.Error, 2 => TraceEventType.Warning, 4 or 5 => TraceEventType.Verbose, _ => TraceEventType.Information };
        Source.TraceEvent(eventType, 0, $"{DateTimeOffset.Now.ToString("O", CultureInfo.InvariantCulture)} {label} [{category}] {message}");
    }
}

internal sealed class RotatingLogListener : TraceListener
{
    private readonly object _gate = new();
    private StreamWriter? _writer;
    private DateOnly _day;
    public override bool IsThreadSafe => true;
    public override void Write(string? message) => WriteLine(message);
    public override void TraceEvent(TraceEventCache? eventCache, string source, TraceEventType eventType, int id, string? message) => WriteLine(message);

    public override void WriteLine(string? message)
    {
        lock (_gate)
        {
            try
            {
                var today = DateOnly.FromDateTime(DateTime.Now);
                if (_writer is null || _day != today || _writer.BaseStream.Length >= 5 * 1024 * 1024)
                {
                    _writer?.Dispose();
                    _writer = null;
                    Directory.CreateDirectory(Path.GetDirectoryName(AppPaths.LogFile)!);
                    if (File.Exists(AppPaths.LogFile))
                    {
                        var info = new FileInfo(AppPaths.LogFile);
                        if (info.Length >= 5 * 1024 * 1024 || DateOnly.FromDateTime(info.LastWriteTime) != today)
                        {
                            File.Move(AppPaths.LogFile, AppPaths.LogFile + "." + DateTime.Now.ToString("yyyyMMdd-HHmmss-fffffff"));
                            var directory = new DirectoryInfo(Path.GetDirectoryName(AppPaths.LogFile)!);
                            foreach (var old in directory.GetFiles(Path.GetFileName(AppPaths.LogFile) + ".*").OrderByDescending(f => f.LastWriteTimeUtc).Skip(10))
                                old.Delete();
                        }
                    }
                    _writer = new StreamWriter(new FileStream(AppPaths.LogFile, FileMode.Append, FileAccess.Write, FileShare.ReadWrite)) { AutoFlush = true };
                    _day = today;
                }
                _writer.WriteLine(message);
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine("日志写入失败: " + ex.Message); }
            try { Console.Error.WriteLine(message); } catch { /* GUI 子系统可能没有终端. */ }
        }
    }

    public override void Flush()
    {
        lock (_gate)
        {
            try
            {
                _writer?.Flush();
                if (_writer?.BaseStream is FileStream stream) stream.Flush(flushToDisk: true);
            }
            catch (Exception ex) { System.Diagnostics.Debug.WriteLine("日志刷盘失败: " + ex.Message); }
        }
    }
}
