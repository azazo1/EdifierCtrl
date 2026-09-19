using System.Text.Json;
using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace EdifierCtrl.Pages;

public sealed partial class GroupPage : Page
{
    private DispatcherTimer? _timer;
    private bool _joined;

    public GroupPage()
    {
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
        _timer.Tick += (_, _) =>
        {
            if (_joined)
            {
                RefreshPeers();
            }
        };
        _timer.Start();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        _timer?.Stop();
        _timer = null;
    }

    private void OnJoin(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.EnsureSession();
            EdifierNative.GroupJoin(Passphrase.Text);
            GroupId.Text = "group_id=" + EdifierNative.GroupIdHex();
            var holding = EdifierNative.Holding();
            Log.Text = string.IsNullOrEmpty(holding)
                ? "已加入组. 点成员接管其耳机."
                : "已加入组. 本机 holding=" + holding;
            _joined = true;
            RefreshPeers();
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message;
        }
    }

    private void OnRefresh(object sender, RoutedEventArgs e) => RefreshPeers();

    private void RefreshPeers()
    {
        try
        {
            Peers.Items.Clear();
            using var doc = JsonDocument.Parse(EdifierNative.GroupPeers());
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                var id = item.GetProperty("id").GetString() ?? "";
                var host = item.TryGetProperty("hostname", out var h) ? h.GetString() ?? id : id;
                var holding = item.TryGetProperty("holding", out var hold) && hold.ValueKind == JsonValueKind.String
                    ? hold.GetString()
                    : null;
                Peers.Items.Add(new PeerItem
                {
                    Id = id,
                    Label = holding is null ? $"{host}  未持有" : $"{host}  holding={holding}",
                });
            }
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message;
        }
    }

    private void OnPeerClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not PeerItem peer)
        {
            return;
        }
        try
        {
            EdifierNative.GroupClaimPeer(peer.Id);
            Log.Text = "已向 " + peer.Label + " 请求接管";
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message;
        }
    }

    private void OnClaim(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.GroupClaim(Mac.Text);
            Log.Text = "已请求接管 " + Mac.Text;
        }
        catch (Exception ex)
        {
            Log.Text = ex.Message;
        }
    }

    private sealed class PeerItem
    {
        public required string Id { get; init; }
        public required string Label { get; init; }
        public override string ToString() => Label;
    }
}
