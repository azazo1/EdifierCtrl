use std::sync::Arc;
use std::time::Duration;

use edifier_protocol::Command;
use edifier_runtime::{CdFallback, GroupHub, HeadsetHost, LinkKind, RuntimeEvent, UdpGroupNet};
use tracing::{info, warn};

use crate::platform::{new_audio, new_headset, PlatformAudio};

pub async fn listen_group(
    passphrase: &str,
    seconds: u64,
    connect: Option<&str>,
    kind: LinkKind,
    claim_peer: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let host = Arc::new(HeadsetHost::new(new_headset()));
    let audio = Arc::new(new_audio());
    let pump = host.clone();
    tokio::spawn(async move {
        loop {
            let _ = pump.pump().await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    let net = UdpGroupNet::bind().await?;
    let id = format!("{}-{}", hostname(), std::process::id());
    let hub = Arc::new(GroupHub::new(
        passphrase,
        id,
        net,
        audio,
        Some(host.clone() as Arc<dyn CdFallback>),
    ));
    info!(
        target: "edifier_cli",
        group = %hub.group_id_hex(),
        seconds,
        "开始听组播"
    );
    println!("group_id={}", hub.group_id_hex());

    let run = hub.clone();
    tokio::spawn(async move {
        let _ = run.run().await;
    });

    let mut events = host.subscribe();
    tokio::spawn(async move {
        while let Ok(ev) = events.recv().await {
            match ev {
                RuntimeEvent::Headset(n) => println!("headset {n:?}"),
                RuntimeEvent::BtState {
                    connected,
                    address,
                    ..
                } => println!(
                    "bt connected={connected} {}",
                    address.unwrap_or_else(|| "-".into())
                ),
                other => info!(target: "edifier_cli", ?other, "运行时事件"),
            }
        }
    });

    if let Some(mac) = connect {
        info!(target: "edifier_cli", mac, "连接控制通道并认领音频");
        host.connect(mac, kind).await?;
        hub.set_control_address(Some(mac.to_string())).await;
        hub.adopt_headset(Some(mac.to_string())).await;
        println!("holding={mac}");
        if let Err(err) = host.send(&Command::QueryBattery).await {
            warn!(target: "edifier_cli", %err, "查电量失败");
        } else {
            info!(target: "edifier_cli", "已发送查电量");
        }
    }

    if let Some(peer_id) = claim_peer {
        tokio::time::sleep(Duration::from_secs(2)).await;
        claim(&hub, peer_id).await?;
    }

    tokio::time::sleep(Duration::from_secs(seconds)).await;
    print_peers(&hub).await;
    Ok(())
}

async fn claim(
    hub: &GroupHub<UdpGroupNet, Arc<PlatformAudio>>,
    peer_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let peers = hub.peers().await;
    let peer = peers
        .iter()
        .find(|p| p.id == peer_id || p.hostname == peer_id)
        .ok_or("组里没有这个成员")?;
    let mac = peer
        .holding
        .as_deref()
        .ok_or("该成员没有持有耳机")?;
    info!(
        target: "edifier_cli",
        peer = %peer.id,
        mac,
        "请求接管"
    );
    hub.claim(mac).await?;
    println!("已向 {} 请求接管 {mac}", peer.hostname);
    Ok(())
}

async fn print_peers(hub: &GroupHub<UdpGroupNet, Arc<PlatformAudio>>) {
    let peers = hub.peers().await;
    if peers.is_empty() {
        println!("这段时间没有发现组员.");
        return;
    }
    for p in peers {
        println!(
            "{} {} holding={}",
            p.id,
            p.hostname,
            p.holding.as_deref().unwrap_or("-")
        );
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}
