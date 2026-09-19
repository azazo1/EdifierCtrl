using EdifierCtrl.Native;
using Microsoft.UI.Xaml;

namespace EdifierCtrl;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        try
        {
            File.WriteAllText(
                Path.Combine(AppContext.BaseDirectory, "launch.log"),
                "App ctor\n");
        }
        catch
        {
            // 忽略启动日志失败.
        }
        UnhandledException += (_, e) =>
        {
            try
            {
                File.AppendAllText(
                    Path.Combine(AppContext.BaseDirectory, "launch.log"),
                    e.Exception.ToString() + Environment.NewLine);
            }
            catch
            {
                // 写日志失败时仍把异常标成已处理, 避免进程直接消失.
            }
            e.Handled = true;
        };
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            File.WriteAllText(
                Path.Combine(AppContext.BaseDirectory, "launch.log"),
                "OnLaunched\n");
            EdifierNative.EnsureSession();
            _window = new MainWindow();
            _window.Closed += (_, _) => EdifierNative.Shutdown();
            _window.Activate();
            _ = Task.Run(SessionState.TryAutoJoin);
            File.AppendAllText(
                Path.Combine(AppContext.BaseDirectory, "launch.log"),
                "activated\n");
        }
        catch (Exception ex)
        {
            File.WriteAllText(
                Path.Combine(AppContext.BaseDirectory, "launch.log"),
                ex.ToString());
            throw;
        }
    }
}
