using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace EdifierCtrl.Pages;

public sealed partial class ControlPage : Page
{
    private EventPump? _pump;

    public ControlPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _pump = new EventPump(ev => Log.Text = ev + "\n" + Log.Text);
        _pump.Start();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        _pump?.Stop();
        _pump = null;
    }

    private void OnNormal(object sender, RoutedEventArgs e) => Send("""{"op":"set_noise_mode","mode":"normal"}""");

    private void OnAnc(object sender, RoutedEventArgs e) => Send("""{"op":"set_noise_mode","mode":"reduction"}""");

    private void OnAmbient(object sender, RoutedEventArgs e) => Send("""{"op":"set_noise_mode","mode":"ambient"}""");

    private void OnBattery(object sender, RoutedEventArgs e) => Send("""{"op":"query_battery"}""");

    private void OnReadout(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.Readout("basedevice");
            Log.Text = "已发送读状态.\n" + Log.Text;
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message;
        }
    }

    private async void OnDisconnectHost(object sender, RoutedEventArgs e)
    {
        var dlg = new ContentDialog
        {
            Title = "确认",
            Content = "发 CD 会断开当前主机. 交接回退可以自动发, 这里是手动.",
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

    private void Send(string json)
    {
        try
        {
            EdifierNative.SendJson(json);
            Log.Text = "已发送 " + json + "\n" + Log.Text;
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message + "\n" + EdifierNative.EncodeCommand(json);
        }
    }
}
