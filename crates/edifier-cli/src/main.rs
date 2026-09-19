mod listen;
mod platform;

use clap::{Parser, Subcommand};
use edifier_protocol::{
    encode_tx, parse_hex, parse_notification, to_hex, verify_checksum, DeviceProfile,
    FrameDecoder, TX_HEAD,
};
use edifier_runtime::{HeadsetTransport, LinkKind};
use edifier_session::readout_plan;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "edifier-cli", about = "漫步者控制协议调试工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 把命令载荷封装成 AA 发送帧.
    Encode { payload: String },
    /// 解开完整收发帧.
    Decode { frame: String },
    /// 解开帧并解析通知.
    Parse { frame: String },
    /// 列出内置机型档案.
    Profiles,
    /// 打印某机型的读设置命令序列.
    Readout { profile: String },
    /// 扫描已配对的漫步者耳机.
    Scan {
        #[arg(long, default_value = "rfcomm")]
        kind: String,
    },
    /// 加入局域网组, 可选连耳机或向某成员请求接管.
    Listen {
        passphrase: String,
        #[arg(long, default_value_t = 8)]
        seconds: u64,
        /// 连接这副耳机并认领 A2DP.
        #[arg(long)]
        connect: Option<String>,
        #[arg(long, default_value = "rfcomm")]
        kind: String,
        /// 成员 id 或主机名, 取其 holding 并发交接.
        #[arg(long)]
        claim_peer: Option<String>,
    },
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let cli = Cli::parse();
    match cli.command {
        Commands::Encode { payload } => {
            let body = parse_hex(&payload)?;
            let frame = encode_tx(&body)?;
            info!(target: "edifier_cli", len = frame.len(), "已封装发送帧");
            println!("{}", to_hex(&frame));
        }
        Commands::Decode { frame } => {
            let raw = parse_hex(&frame)?;
            if raw.first() == Some(&TX_HEAD) {
                verify_checksum(&raw)?;
                let end = raw.len().saturating_sub(2);
                let body = if end >= 2 { &raw[2..end] } else { &[] };
                println!("{}", to_hex(body));
            } else {
                let mut dec = FrameDecoder::new();
                for item in dec.push(&raw) {
                    let f = item?;
                    println!("head {:02X} body {}", f.head, to_hex(&f.body));
                }
            }
        }
        Commands::Parse { frame } => {
            let raw = parse_hex(&frame)?;
            let mut dec = FrameDecoder::new();
            for item in dec.push(&raw) {
                let f = item?;
                println!("{:?}", parse_notification(&f));
            }
        }
        Commands::Profiles => {
            for p in DeviceProfile::all() {
                println!(
                    "{} {} uuid={} name_len={}",
                    p.id.as_str(),
                    p.display_name,
                    p.unique_service_uuid.unwrap_or("-"),
                    p.max_name_len
                );
            }
        }
        Commands::Readout { profile } => {
            let p = DeviceProfile::by_key(&profile).ok_or("未知机型档案")?;
            for cmd in readout_plan(p) {
                let body = cmd.to_body()?;
                println!("{} {}", cmd.label(), to_hex(&body));
            }
        }
        Commands::Scan { kind } => {
            let kind = parse_kind(&kind)?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(scan_devices(kind))?;
        }
        Commands::Listen {
            passphrase,
            seconds,
            connect,
            kind,
            claim_peer,
        } => {
            let kind = parse_kind(&kind)?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(listen::listen_group(
                &passphrase,
                seconds,
                connect.as_deref(),
                kind,
                claim_peer.as_deref(),
            ))?;
        }
    }
    Ok(())
}

fn parse_kind(kind: &str) -> Result<LinkKind, Box<dyn std::error::Error>> {
    match kind {
        "rfcomm" => Ok(LinkKind::Rfcomm),
        "ble" => Ok(LinkKind::Ble),
        _ => Err("kind 只能是 rfcomm 或 ble".into()),
    }
}

#[cfg(windows)]
async fn scan_devices(kind: LinkKind) -> Result<(), Box<dyn std::error::Error>> {
    let hs = edifier_bt_windows::WindowsHeadset::new();
    print_scan(hs.scan(kind).await?)
}

#[cfg(target_os = "linux")]
async fn scan_devices(kind: LinkKind) -> Result<(), Box<dyn std::error::Error>> {
    let hs = edifier_bt_linux::LinuxHeadset::new();
    print_scan(hs.scan(kind).await?)
}

#[cfg(target_os = "macos")]
async fn scan_devices(kind: LinkKind) -> Result<(), Box<dyn std::error::Error>> {
    let hs = edifier_bt_macos::MacosHeadset::new();
    print_scan(hs.scan(kind).await?)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
async fn scan_devices(_kind: LinkKind) -> Result<(), Box<dyn std::error::Error>> {
    Err("本平台尚未接入扫描".into())
}

fn print_scan(list: Vec<edifier_runtime::ScanResult>) -> Result<(), Box<dyn std::error::Error>> {
    if list.is_empty() {
        println!("没有发现设备. 请先在系统里配对.");
        return Ok(());
    }
    for d in list {
        println!(
            "{} {} {:?} {}",
            d.address,
            d.name,
            d.kind,
            d.service_uuid.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
