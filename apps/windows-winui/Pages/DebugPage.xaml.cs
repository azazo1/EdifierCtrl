using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl.Pages;

public sealed partial class DebugPage : Page
{
    public DebugPage()
    {
        InitializeComponent();
        Payload.Text = """{"op":"query_battery"}""";
    }

    private void OnEncode(object sender, RoutedEventArgs e)
    {
        try
        {
            Result.Text = EdifierNative.EncodeCommand(Payload.Text);
        }
        catch (Exception ex)
        {
            Result.Text = ex.Message;
        }
    }

    private void OnParse(object sender, RoutedEventArgs e)
    {
        try
        {
            Result.Text = EdifierNative.ParseFrame(Payload.Text);
        }
        catch (Exception ex)
        {
            Result.Text = ex.Message;
        }
    }

    private void OnPoll(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.EnsureSession();
            Result.Text = EdifierNative.PollEvent();
        }
        catch (Exception ex)
        {
            Result.Text = ex.Message;
        }
    }
}
