namespace EdifierCtrl.Infrastructure;

internal static class PreferenceMigrations
{
    public static void Validate(AppPreferences preferences)
    {
        if (preferences.SchemaVersion != 1)
            throw new InvalidDataException($"不支持设置格式版本 {preferences.SchemaVersion}.");
        if (string.IsNullOrWhiteSpace(preferences.InstallationId))
            preferences.InstallationId = Guid.NewGuid().ToString("N");
        if (preferences.Theme is not ("system" or "light" or "dark")) preferences.Theme = "system";
        if (!double.IsFinite(preferences.WindowWidth) || preferences.WindowWidth < 400) preferences.WindowWidth = 1120;
        if (!double.IsFinite(preferences.WindowHeight) || preferences.WindowHeight < 300) preferences.WindowHeight = 780;
        if (!preferences.RememberGroup) preferences.GroupName = "";
        preferences.GroupName ??= "";
        preferences.SelectedProfile ??= "basedevice";
        preferences.LastDeviceAddress ??= "";
        preferences.LastDeviceName ??= "";
    }
}
