using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;

namespace EdifierCtrl.Pages;

public sealed partial class DebugPage : Page
{
    private bool _initialized;
    private bool _subscribed;
    private bool _working;
    private string _activityKey = "";

    public DebugPage()
    {
        InitializeComponent();
        Payload.Text = """{"op":"query_battery"}""";
        _initialized = true;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private static bool CanDiagnose => SessionState.CanControl && !SessionState.Busy;

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        if (!_subscribed)
        {
            SessionState.Changed += OnState;
            _subscribed = true;
        }
        OnState();
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        SessionState.Changed -= OnState;
        _subscribed = false;
    }

    private void OnState()
    {
        if (!_initialized)
        {
            return;
        }
        var next = SessionState.Activities.Select(entry => new ActivityRow
        {
            TimeText = entry.Time.ToLocalTime().ToString("HH:mm:ss"),
            FullTime = entry.Time.ToLocalTime().ToString("yyyy-MM-dd HH:mm:ss zzz"),
            Title = entry.Title,
            Detail = entry.Detail,
            IsError = entry.IsError,
        }).ToArray();
        var key = string.Join("\n", SessionState.Activities.Select(entry => $"{entry.Time:O}\t{entry.Title}\t{entry.Detail}\t{entry.IsError}"));
        if (_activityKey != key)
        {
            _activityKey = key;
            ActivitiesList.ItemsSource = next;
        }
        ActivitiesEmpty.Visibility = PageUi.Visible(next.Length == 0);
        ClearButton.IsEnabled = next.Length > 0;
        UpdateDiagnosticButtons();
    }

    private void UpdateDiagnosticButtons()
    {
        if (!_initialized)
        {
            return;
        }
        EncodeButton.IsEnabled = ParseButton.IsEnabled = SendButton.IsEnabled = CanDiagnose && !_working && !string.IsNullOrWhiteSpace(Payload.Text);
        DiagnosticState.Text = _working ? "正在处理诊断操作..." : CanDiagnose ? "控制通道就绪" : "等待连接可控耳机, 并完成当前操作";
    }

    private void OnPayloadChanged(object sender, TextChangedEventArgs e) => UpdateDiagnosticButtons();
    private void OnClear(object sender, RoutedEventArgs e) => SessionState.ClearActivities();
    private async void OnEncode(object sender, RoutedEventArgs e) => await InspectAsync(false);
    private async void OnParse(object sender, RoutedEventArgs e) => await InspectAsync(true);

    private async Task InspectAsync(bool parse)
    {
        if (!CanDiagnose || _working || string.IsNullOrWhiteSpace(Payload.Text))
        {
            return;
        }
        _working = true;
        UpdateDiagnosticButtons();
        try
        {
            ShowResult(await AppActions.DiagnosticAsync(Payload.Text, parse));
        }
        finally
        {
            _working = false;
            UpdateDiagnosticButtons();
        }
    }

    private async void OnSend(object sender, RoutedEventArgs e)
    {
        if (!CanDiagnose || _working || string.IsNullOrWhiteSpace(Payload.Text))
        {
            return;
        }
        _working = true;
        UpdateDiagnosticButtons();
        try
        {
            using var document = JsonDocument.Parse(Payload.Text);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("op", out var operation)
                || operation.ValueKind != JsonValueKind.String || string.IsNullOrWhiteSpace(operation.GetString()))
            {
                ShowResult("请输入包含非空字符串 op 字段的 JSON 对象.");
                return;
            }
            var op = operation.GetString()!;
            var values = new Dictionary<string, JsonElement>();
            foreach (var property in root.EnumerateObject())
            {
                if (property.Name != "op")
                {
                    values[property.Name] = property.Value.Clone();
                }
            }
            if (!await PageUi.ConfirmAsync(this, "发送诊断命令", $"将向当前耳机发送 {op}. 此操作会实际改变耳机状态, 关机或恢复出厂等命令可能中断连接或清除配对记录.", "发送命令")
                || !CanDiagnose)
            {
                return;
            }
            await AppActions.SendAsync(op, values, "诊断命令: " + op);
            ShowResult(SessionState.NoticeIsError
                ? (SessionState.NoticeTitle ?? "发送操作失败") + "\n" + SessionState.NoticeDetail
                : "发送操作已结束. 请查看活动记录与耳机状态回报.");
        }
        catch (JsonException error)
        {
            ShowResult("JSON 无法解析: " + error.Message);
        }
        finally
        {
            _working = false;
            UpdateDiagnosticButtons();
        }
    }

    private void ShowResult(string result)
    {
        Result.Text = result;
        ResultCard.Visibility = PageUi.Visible(!string.IsNullOrEmpty(result));
    }

    [Bindable]
    public sealed class ActivityRow
    {
        public string TimeText { get; set; } = "";
        public string FullTime { get; set; } = "";
        public string Title { get; set; } = "";
        public string Detail { get; set; } = "";
        public bool IsError { get; set; }
        public Visibility ErrorVisibility => PageUi.Visible(IsError);
        public Visibility NormalVisibility => PageUi.Visible(!IsError);
        public Visibility DetailVisibility => PageUi.Visible(!string.IsNullOrWhiteSpace(Detail));
    }
}
