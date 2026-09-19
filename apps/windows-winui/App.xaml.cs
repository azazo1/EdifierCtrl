using EdifierCtrl.Desktop;
using EdifierCtrl.Infrastructure;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace EdifierCtrl;

public partial class App : Application
{
    public App()
    {
        UnhandledException += (_, e) =>
        {
            AppLog.Error(e.Exception.ToString(), "unhandled");
            AppLog.Flush();
        };
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            AppLog.Error(e.ExceptionObject.ToString() ?? "未知异常", "unhandled");
            AppLog.Flush();
        };
        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            AppLog.Error(e.Exception.ToString(), "task");
            AppLog.Flush();
            e.SetObserved();
        };
        InitializeComponent();
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            var desktop = new DesktopController(DispatcherQueue.GetForCurrentThread());
            DesktopCommands.Controller = desktop;
            await desktop.StartAsync(args.Arguments);
        }
        catch (Exception ex)
        {
            AppLog.Error("应用启动失败: " + ex, "desktop");
            AppLog.Flush();
            Exit();
        }
    }
}
