mod state;
mod ui;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    let app = Application::builder()
        .application_id("dev.edifierctrl.app")
        .build();
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("EdifierCtrl")
        .default_width(800)
        .default_height(520)
        .child(&ui::root())
        .build();
    window.present();
}
