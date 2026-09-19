using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class ControlPage : Page
{
    public ControlPage()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            Lead.Text = SessionState.Connected
                ? "改降噪或查电量会发到已连接的耳机."
                : "先到设备页连接耳机.";
        };
    }

    private void OnModeClick(object sender, RoutedEventArgs e)
    {
        if (sender is not RadioButton rb || rb.Tag is not string mode)
        {
            return;
        }
        Send("{\"op\":\"set_noise_mode\",\"mode\":\"" + mode + "\"}");
    }

    private void OnBattery(object sender, RoutedEventArgs e) => Send("""{"op":"query_battery"}""");

    private void OnReadout(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.Readout("basedevice");
            SessionState.SetHint("已请求读取状态");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private async void OnDisconnectHost(object sender, RoutedEventArgs e)
    {
        var dlg = new ContentDialog
        {
            Title = "断开当前主机",
            Content = "会给耳机发 CD, 当前正在播放的设备会掉线. 局域网交接失败时会自动发, 这里是手动.",
            PrimaryButtonText = "发送",
            CloseButtonText = "取消",
            XamlRoot = XamlRoot,
        };
        if (await dlg.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        Send("""{"op":"disconnect_host"}""");
    }

    private static void Send(string json)
    {
        try
        {
            EdifierNative.SendJson(json);
            SessionState.SetHint("已发送");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }
}
