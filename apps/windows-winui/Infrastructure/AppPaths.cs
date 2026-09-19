using System.Reflection;

namespace EdifierCtrl.Infrastructure;

internal static class AppPaths
{
    public static string DataDirectory { get; } = Path.GetFullPath(
        Environment.GetEnvironmentVariable("EDIFIER_DATA_DIR")
        ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "EdifierCtrl"));

    public static string LogFile { get; } = Path.GetFullPath(
        Environment.GetEnvironmentVariable("EDIFIER_LOG_FILE") ?? Path.Combine(DataDirectory, "app.log"));

    public static string Version { get; } = typeof(AppPaths).Assembly
        .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion.Split('+')[0] ?? "dev-build";
}
