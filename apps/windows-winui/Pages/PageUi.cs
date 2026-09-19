using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace EdifierCtrl.Pages;

internal static class PageUi
{
    public static Visibility Visible(bool value) => value ? Visibility.Visible : Visibility.Collapsed;

    public static bool HasFocus(FrameworkElement element)
    {
        if (element.XamlRoot is null)
        {
            return false;
        }
        var current = FocusManager.GetFocusedElement(element.XamlRoot) as DependencyObject;
        while (current is not null)
        {
            if (ReferenceEquals(current, element))
            {
                return true;
            }
            current = VisualTreeHelper.GetParent(current);
        }
        return false;
    }

    public static string? NormalizeAddress(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }
        var compact = value.Trim().Replace(":", "").Replace("-", "");
        return compact.Length == 12 && compact.All(Uri.IsHexDigit) ? compact.ToUpperInvariant() : null;
    }

    public static bool SameAddress(string? left, string? right) =>
        NormalizeAddress(left) is { } address && address == NormalizeAddress(right);

    public static async Task<bool> ConfirmAsync(FrameworkElement owner, string title, string detail, string action)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = owner.XamlRoot,
            RequestedTheme = owner.ActualTheme,
            Title = title,
            Content = detail,
            PrimaryButtonText = action,
            CloseButtonText = "取消",
            DefaultButton = ContentDialogButton.Close,
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }
}

// 只有确认读数发生变化才安排同步. 正在编辑时延后同步, 保留未提交草稿.
internal sealed class ReadingDraft<T>
{
    private bool _observed;
    private bool _pending;
    private T? _lastReading;

    public void Reset()
    {
        _observed = false;
        _pending = false;
        _lastReading = default;
    }

    public void Synchronize(T reading, bool editing, Action<T> apply)
    {
        if (!_observed || !EqualityComparer<T>.Default.Equals(reading, _lastReading!))
        {
            _observed = true;
            _lastReading = reading;
            _pending = true;
        }
        if (!_pending || editing)
        {
            return;
        }
        _pending = false;
        apply(reading);
    }
}
