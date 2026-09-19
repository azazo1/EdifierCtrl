using EdifierCtrl.Native;
using Microsoft.UI.Xaml;

namespace EdifierCtrl;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            EdifierNative.EnsureSession();
        }
        catch
        {
            // 没有 dll 时界面仍可打开, 页面会提示.
        }
        _window = new MainWindow();
        _window.Closed += (_, _) => EdifierNative.Shutdown();
        _window.Activate();
    }
}
