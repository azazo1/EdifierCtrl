using System.Text.Json;
using System.Text.Json.Serialization;

namespace EdifierCtrl.Native;

[Microsoft.UI.Xaml.Data.Bindable]
public sealed record HeadphoneProfile
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";
    [JsonPropertyName("display_name")]
    public string DisplayName { get; set; } = "";
    [JsonPropertyName("features")]
    public IReadOnlyList<string> Capabilities { get; set; } = [];
    [JsonPropertyName("max_name_len")]
    public int MaxNameLen { get; set; }
    [JsonPropertyName("unique_service_uuid")]
    public string? ServiceUuid { get; set; }

    public bool Supports(string feature) => Capabilities.Contains(feature, StringComparer.Ordinal);
}

public sealed record HeadphoneDevice
{
    [JsonPropertyName("address")]
    public string Address { get; init; } = "";
    [JsonPropertyName("name")]
    public string Name { get; init; } = "";
    [JsonPropertyName("kind")]
    public string Kind { get; init; } = "rfcomm";
    [JsonPropertyName("service_uuid")]
    public string? ServiceUuid { get; init; }
    [JsonIgnore]
    public string? PeerId { get; init; }
    [JsonIgnore]
    public string DisplayName => string.IsNullOrWhiteSpace(Name) ? "未命名耳机" : Name;
    [JsonIgnore]
    public string ActionText { get; init; } = "连接控制";
}

public sealed record GroupPeer
{
    [JsonPropertyName("id")]
    public string Id { get; init; } = "";
    [JsonPropertyName("hostname")]
    public string Host { get; init; } = "";
    [JsonPropertyName("holding")]
    public string? Holding { get; init; }
    [JsonPropertyName("os")]
    public string Platform { get; init; } = "";
    [JsonPropertyName("app_version")]
    public string AppVersion { get; init; } = "";
    [JsonPropertyName("can_audio")]
    public bool CanAudio { get; init; }
    [JsonIgnore]
    public string DisplayName => string.IsNullOrWhiteSpace(Host) ? Id : Host;
}

public sealed record ActivityEntry(DateTimeOffset Time, string Title, string Detail, bool IsError);

internal sealed record NativeSnapshot(IReadOnlyList<JsonElement> Events, IReadOnlyList<GroupPeer>? Peers, string? Holding);

internal static class NativeJson
{
    internal static T Decode<T>(string json) => JsonSerializer.Deserialize<T>(json)
        ?? throw new JsonException("耳机服务返回了空数据.");

    internal static string? Text(JsonElement value, string key) =>
        value.TryGetProperty(key, out var item) && item.ValueKind == JsonValueKind.String ? item.GetString() : null;

    internal static bool? Boolean(JsonElement value, string key) =>
        value.TryGetProperty(key, out var item) && item.ValueKind is JsonValueKind.True or JsonValueKind.False ? item.GetBoolean() : null;

    internal static int? Integer(JsonElement value, string key) =>
        value.TryGetProperty(key, out var item) && item.ValueKind == JsonValueKind.Number && item.TryGetInt32(out var number) ? number : null;
}
