using System.Text.Json;
using System.Text.Json.Serialization;

namespace EdifierCtrl.Infrastructure;

internal sealed class AppPreferences
{
    public static AppPreferences Current { get; } = Load();
    public static event Action? Changed;
    public int SchemaVersion { get; set; } = 1;
    public string InstallationId { get; set; } = Guid.NewGuid().ToString("N");
    public string GroupName { get; set; } = "";
    public bool RememberGroup { get; set; } = true;
    public bool AutoJoinGroup { get; set; } = true;
    public string SelectedProfile { get; set; } = "basedevice";
    public string LastDeviceAddress { get; set; } = "";
    public string LastDeviceName { get; set; } = "";
    public bool VerboseLogging { get; set; }
    public bool StartHidden { get; set; }
    public string Theme { get; set; } = "system";
    public double WindowWidth { get; set; } = 1120;
    public double WindowHeight { get; set; } = 780;
    public bool Maximized { get; set; }
    [JsonIgnore] public string? SaveError { get; private set; }

    private static string FilePath => Path.Combine(AppPaths.DataDirectory, "preferences.json");

    private static AppPreferences Load()
    {
        try
        {
            if (File.Exists(FilePath))
            {
                var preferences = JsonSerializer.Deserialize<AppPreferences>(File.ReadAllText(FilePath));
                if (preferences is not null)
                {
                    PreferenceMigrations.Validate(preferences);
                    return preferences;
                }
            }
        }
        catch (Exception ex) { AppLog.Error($"无法读取设置, 使用默认值: {ex.Message}", "preferences"); }
        return new AppPreferences();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(AppPaths.DataDirectory);
            var temp = FilePath + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }));
            File.Move(temp, FilePath, overwrite: true);
            SaveError = null;
            AppLog.SetVerbose(VerboseLogging);
            AppLog.Debug("设置已保存.", "preferences");
        }
        catch (Exception ex)
        {
            SaveError = "无法保存设置: " + ex.Message;
            AppLog.Error(SaveError, "preferences");
        }
        Changed?.Invoke();
    }
}
