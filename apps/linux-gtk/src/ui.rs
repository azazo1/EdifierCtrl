use std::sync::Arc;
use std::time::Duration;

use edifier_group::PeerInfo;
use edifier_protocol::{encode_tx, to_hex, Command, NoiseMode};
use edifier_runtime::LinkKind;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Box, Button, Entry, Label, ListBox, ListBoxRow, Orientation};
use tracing::info;

use crate::state::AppState;

pub fn root() -> gtk::Box {
    let state = AppState::new();
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.add_titled(&device_page(state.clone()), "device", "设备");
    stack.add_titled(&control_page(state.clone()), "control", "控制");
    stack.add_titled(&group_page(state.clone()), "group", "组");
    stack.add_titled(&debug_page(), "debug", "调试");
    let events = Label::new(Some("事件"));
    events.set_wrap(true);
    events.set_xalign(0.0);
    let st_ev = state;
    let ev_label = events.clone();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        if let Some(text) = st_ev.poll_event() {
            let old = ev_label.text();
            ev_label.set_text(&format!("{text}\n{old}"));
        }
        glib::ControlFlow::Continue
    });
    let sidebar = gtk::StackSidebar::new();
    sidebar.set_stack(&stack);
    let body = gtk::Box::new(Orientation::Vertical, 8);
    body.append(&stack);
    body.append(&events);
    let root = gtk::Box::new(Orientation::Horizontal, 0);
    root.append(&sidebar);
    root.append(&body);
    root
}

fn device_page(state: Arc<AppState>) -> Box {
    let log = Label::new(Some("扫描已配对耳机, 点列表连接."));
    log.set_wrap(true);
    log.set_xalign(0.0);
    let list = ListBox::new();
    let page = col();
    page.append(&title("设备"));
    page.append(&log);
    page.append(&scan_btn(
        state.clone(),
        log.clone(),
        list.clone(),
        LinkKind::Rfcomm,
        "扫描 RFCOMM",
    ));
    page.append(&scan_btn(
        state.clone(),
        log.clone(),
        list.clone(),
        LinkKind::Ble,
        "扫描 BLE",
    ));
    let st_row = state.clone();
    let log_row = log.clone();
    list.connect_row_activated(move |_, row| {
        let address = row.widget_name().to_string();
        match st_row.connect(&address) {
            Ok(()) => log_row.set_text(&format!("已连接 {address}")),
            Err(err) => log_row.set_text(&err),
        }
    });
    page.append(&list);
    let disc = Button::with_label("断开控制");
    let st_d = state;
    let log_d = log.clone();
    disc.connect_clicked(move |_| {
        match st_d.disconnect() {
            Ok(()) => log_d.set_text("已断开控制通道."),
            Err(err) => log_d.set_text(&err),
        }
    });
    page.append(&disc);
    page.append(&log);
    page
}

fn scan_btn(
    state: Arc<AppState>,
    log: Label,
    list: ListBox,
    kind: LinkKind,
    label: &str,
) -> Button {
    let btn = Button::with_label(label);
    btn.connect_clicked(move |_| {
        info!(target: "edifier_linux", ?kind, "扫描");
        match state.scan(kind) {
            Ok(devices) if devices.is_empty() => {
                fill_devices(&list, &[]);
                log.set_text("没有发现设备. 请先在系统里配对.");
            }
            Ok(devices) => {
                fill_devices(&list, &devices);
                log.set_text("选中后连接.");
            }
            Err(err) => log.set_text(&err),
        }
    });
    btn
}

fn fill_devices(list: &ListBox, devices: &[edifier_runtime::ScanResult]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for d in devices {
        let row = ListBoxRow::new();
        row.set_widget_name(&d.address);
        let label = Label::new(Some(&format!("{}  {}", d.address, d.name)));
        label.set_xalign(0.0);
        row.set_child(Some(&label));
        list.append(&row);
    }
}

fn control_page(state: Arc<AppState>) -> Box {
    let log = Label::new(Some("先在设备页连接耳机."));
    log.set_wrap(true);
    log.set_xalign(0.0);
    let page = col();
    page.append(&title("控制"));
    page.append(&cmd_btn(
        state.clone(),
        log.clone(),
        "降噪关",
        Command::SetNoiseMode(NoiseMode::Normal),
    ));
    page.append(&cmd_btn(
        state.clone(),
        log.clone(),
        "降噪",
        Command::SetNoiseMode(NoiseMode::Reduction),
    ));
    page.append(&cmd_btn(
        state.clone(),
        log.clone(),
        "查电量",
        Command::QueryBattery,
    ));
    let read = Button::with_label("读取状态");
    let st_r = state;
    let log_r = log.clone();
    read.connect_clicked(move |_| {
        match st_r.readout() {
            Ok(()) => log_r.set_text("已发送读状态命令."),
            Err(err) => log_r.set_text(&err),
        }
    });
    page.append(&read);
    page.append(&cd_btn(state, log.clone()));
    page.append(&log);
    page
}

fn cd_btn(state: Arc<AppState>, log: Label) -> Button {
    let btn = Button::with_label("断开主机 (CD)");
    btn.connect_clicked(move |btn| {
        let window = btn.root().and_downcast::<gtk::Window>();
        let dlg = gtk::AlertDialog::builder()
            .message("确认")
            .detail_text("发 CD 会断开当前主机. 交接回退可以自动发, 这里是手动.")
            .buttons(["取消", "发送"])
            .cancel_button(0)
            .default_button(1)
            .modal(true)
            .build();
        let st = state.clone();
        let log = log.clone();
        dlg.choose(window.as_ref(), None::<&gtk::gio::Cancellable>, move |res| {
            if res.ok() != Some(1) {
                return;
            }
            match st.send(&Command::DisconnectHost) {
                Ok(()) => log.set_text("已发送 Disconnect"),
                Err(err) => log.set_text(&err),
            }
        });
    });
    btn
}

fn cmd_btn(state: Arc<AppState>, log: Label, label: &str, cmd: Command) -> Button {
    let btn = Button::with_label(label);
    btn.connect_clicked(move |_| {
        match state.send(&cmd) {
            Ok(()) => {
                info!(target: "edifier_linux", label = cmd.label(), "已发送");
                log.set_text(&format!("已发送 {}", cmd.label()));
            }
            Err(_) => match cmd.to_body().and_then(|b| encode_tx(&b)) {
                Ok(frame) => log.set_text(&to_hex(&frame)),
                Err(err) => log.set_text(&err.to_string()),
            },
        }
    });
    btn
}

fn group_page(state: Arc<AppState>) -> Box {
    let log = Label::new(Some("加入后点某个成员, 接管其正在持有的耳机."));
    log.set_wrap(true);
    log.set_xalign(0.0);
    let pass = Entry::new();
    pass.set_placeholder_text(Some("组名"));
    let mac = Entry::new();
    mac.set_placeholder_text(Some("耳机 MAC"));
    let list = ListBox::new();
    let page = col();
    page.append(&title("组"));
    page.append(&pass);
    let join = Button::with_label("加入");
    let st_join = state.clone();
    let log_join = log.clone();
    let pass_join = pass.clone();
    join.connect_clicked(move |_| {
        let phrase = pass_join.text().to_string();
        match st_join.join_group(&phrase) {
            Ok(gid) => log_join.set_text(&format!("group_id={gid}")),
            Err(err) => log_join.set_text(&err),
        }
    });
    page.append(&join);
    let refresh = Button::with_label("刷新成员");
    let st_ref = state.clone();
    let list_ref = list.clone();
    let log_ref = log.clone();
    refresh.connect_clicked(move |_| {
        match st_ref.peers() {
            Ok(peers) => fill_peers(&list_ref, &peers),
            Err(err) => log_ref.set_text(&err),
        }
    });
    page.append(&refresh);
    let st_row = state.clone();
    let log_row = log.clone();
    list.connect_row_activated(move |_, row| {
        let peer_id = row.widget_name().to_string();
        match st_row.claim_peer(&peer_id) {
            Ok(msg) => log_row.set_text(&msg),
            Err(err) => log_row.set_text(&err),
        }
    });
    page.append(&list);
    page.append(&mac);
    let claim = Button::with_label("接管音频");
    let st_c = state;
    let log_c = log.clone();
    let mac_c = mac;
    claim.connect_clicked(move |_| {
        let address = mac_c.text().to_string();
        match st_c.claim(&address) {
            Ok(()) => log_c.set_text(&format!("已请求接管 {address}")),
            Err(err) => log_c.set_text(&err),
        }
    });
    page.append(&claim);
    page.append(&log);
    page
}

fn fill_peers(list: &ListBox, peers: &[PeerInfo]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for peer in peers {
        let row = ListBoxRow::new();
        row.set_widget_name(&peer.id);
        let holding = peer.holding.as_deref().unwrap_or("未持有");
        let label = Label::new(Some(&format!("{}  {holding}", peer.hostname)));
        label.set_xalign(0.0);
        row.set_child(Some(&label));
        list.append(&row);
    }
}

fn debug_page() -> Box {
    let payload = Entry::new();
    payload.set_placeholder_text(Some("命令 body hex, 如 C101"));
    payload.set_text("C101");
    let log = Label::new(None);
    log.set_wrap(true);
    log.set_xalign(0.0);
    let encode = Button::with_label("封装");
    let payload_c = payload.clone();
    let log_c = log.clone();
    encode.connect_clicked(move |_| {
        match edifier_protocol::parse_hex(&payload_c.text()).and_then(|b| encode_tx(&b)) {
            Ok(frame) => log_c.set_text(&to_hex(&frame)),
            Err(err) => log_c.set_text(&err.to_string()),
        }
    });
    let page = col();
    page.append(&title("调试"));
    page.append(&payload);
    page.append(&encode);
    page.append(&log);
    page
}

fn col() -> Box {
    let page = Box::new(Orientation::Vertical, 8);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(24);
    page.set_margin_end(24);
    page
}

fn title(text: &str) -> Label {
    let l = Label::new(Some(text));
    l.set_xalign(0.0);
    l.add_css_class("title-1");
    l
}
