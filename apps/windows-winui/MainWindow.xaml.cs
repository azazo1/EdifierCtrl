using EdifierCtrl.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace EdifierCtrl;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        Title = "EdifierCtrl";
        Nav.SelectedItem = Nav.MenuItems[0];
        ContentFrame.Content = new DevicePage();
    }

    private void OnNav(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag)
        {
            return;
        }
        ContentFrame.Content = tag switch
        {
            "control" => new ControlPage(),
            "group" => new GroupPage(),
            "debug" => new DebugPage(),
            _ => new DevicePage(),
        };
    }
}
