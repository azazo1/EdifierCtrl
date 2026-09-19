using EdifierCtrl.Native;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage;

namespace EdifierCtrl.Pages;

public sealed partial class GroupPage : Page
{
    private DispatcherTimer? _timer;
    private string _peerKey = "";

    public GroupPage()
    {
        InitializeComponent();
        try
        {
            var saved = ApplicationData.Current.LocalSettings.Values["passphrase"] as string;
            if (!string.IsNullOrEmpty(saved))
            {
                Passphrase.Text = saved;
            }
        }
        catch
        {
            // unpackaged 仍可用 LocalSettings; 失败就空着.
        }
        Loaded += (_, _) =>
        {
            _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(2) };
            _timer.Tick += (_, _) =>
            {
                if (SessionState.GroupJoined)
                {
                    SessionState.RefreshPeers();
                    ShowPeers();
                }
            };
            _timer.Start();
            if (SessionState.GroupJoined)
            {
                JoinBtn.Content = "已加入";
                RefreshPeers();
            }
        };
        Unloaded += (_, _) =>
        {
            _timer?.Stop();
            _timer = null;
        };
    }

    private void OnJoin(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.EnsureSession();
            EdifierNative.GroupJoin(Passphrase.Text);
            try
            {
                ApplicationData.Current.LocalSettings.Values["passphrase"] = Passphrase.Text;
            }
            catch
            {
                // 记不住口令也不挡加入.
            }
            SessionState.GroupJoined = true;
            SessionState.Holding = EdifierNative.Holding();
            JoinBtn.Content = "已加入";
            SessionState.SetHint("已加入组. 点成员接管其耳机.");
            SessionState.RefreshPeers();
            RefreshPeers();
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private void OnRefresh(object sender, RoutedEventArgs e) => RefreshPeers();

    private void RefreshPeers()
    {
        SessionState.RefreshPeers();
        ShowPeers();
    }

    private void ShowPeers()
    {
        var next = SessionState.Peers
            .Select(p => new PeerItem
            {
                Id = p.Id,
                Host = p.Host,
                Holding = p.Holding,
                HoldingText = string.IsNullOrEmpty(p.Holding) ? "未持有耳机" : "持有 " + p.Holding,
                ActionText = string.IsNullOrEmpty(p.Holding) ? "无法接管" : "点按接管音频",
            })
            .ToList();
        var key = string.Join("|", next.Select(p => p.Id + "\t" + p.Host + "\t" + p.Holding));
        if (key == _peerKey)
        {
            return;
        }
        _peerKey = key;
        Peers.ItemsSource = next;
    }

    private async void OnPeerClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is not PeerItem peer)
        {
            return;
        }
        if (string.IsNullOrEmpty(peer.Holding))
        {
            var dlg = new ContentDialog
            {
                Title = peer.Host,
                Content = "这个成员现在没有持有耳机, 没法接管.",
                CloseButtonText = "好",
                XamlRoot = XamlRoot,
            };
            await dlg.ShowAsync();
            return;
        }
        try
        {
            EdifierNative.GroupClaimPeer(peer.Id);
            SessionState.SetHint("已向 " + peer.Host + " 请求接管");
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private void OnClaim(object sender, RoutedEventArgs e)
    {
        try
        {
            EdifierNative.GroupClaim(Mac.Text);
            SessionState.SetHint("已请求接管 " + Mac.Text);
        }
        catch (Exception ex)
        {
            SessionState.SetHint(ex.Message);
        }
    }

    private sealed class PeerItem
    {
        public required string Id { get; init; }
        public required string Host { get; init; }
        public string? Holding { get; init; }
        public required string HoldingText { get; init; }
        public required string ActionText { get; init; }
    }
}
