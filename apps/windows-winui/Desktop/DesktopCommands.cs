using System.Diagnostics;
using EdifierCtrl.Infrastructure;

namespace EdifierCtrl.Desktop;

internal static class DesktopCommands
{
    public static DesktopController? Controller { get; set; }
    public static void Navigate(string key) => Controller?.Navigate(key);
    public static void Quit() => Controller?.RequestQuit("用户退出");
    public static void ApplyTheme() => Controller?.ApplyTheme();
    public static void OpenBluetoothSettings() => Open("ms-settings:bluetooth");
    public static void OpenDataDirectory()
    {
        Directory.CreateDirectory(AppPaths.DataDirectory);
        Open(AppPaths.DataDirectory);
    }
    public static void Open(string target)
    {
        try { Process.Start(new ProcessStartInfo(target) { UseShellExecute = true }); }
        catch (Exception ex) { AppLog.Warn("无法打开系统位置: " + ex.Message, "desktop"); }
    }
}
