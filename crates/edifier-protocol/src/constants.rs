/// 发送帧头.
pub const TX_HEAD: u8 = 0xAA;
/// 设备响应帧头.
pub const RX_HEAD_BB: u8 = 0xBB;
/// 设备确认帧头.
pub const RX_HEAD_CC: u8 = 0xCC;
/// 校验和初值.
pub const CHECKSUM_INIT: u16 = 8217;

/// SPP/RFCOMM 服务 UUID.
pub const RFCOMM_SERVICE_UUID: &str = "edf00000-edfe-dfed-fedf-edfedfedfedf";
/// 多数机型 BLE service UUID 的固定后缀.
pub const BLE_SERVICE_UUID_SUFFIX: &str = "-1a48-11e9-ab14-d663bd873d93";
/// BLE 写特征分片长度.
pub const BLE_CHUNK_LEN: usize = 20;
/// BLE 分片间隔, 毫秒.
pub const BLE_CHUNK_DELAY_MS: u64 = 50;
/// 接收缓冲超时, 毫秒.
pub const RX_BUFFER_TIMEOUT_MS: u64 = 5000;
/// 连续命令间隔, 毫秒.
pub const COMMAND_GAP_MS: u64 = 150;
/// 发 CD 后等待再连音频, 毫秒.
pub const AUDIO_CONNECT_GAP_MS: u64 = 2500;
/// 环境声音量编码偏移, 实际音量 = 编码值 - 6.
pub const AMBIENT_VOLUME_OFFSET: i8 = 6;

pub const BLE_RX_UUID_HINT: &str = "2-1a48-11e9-ab14-d663bd873d93";
pub const BLE_TX_UUID_HINT: &str = "3-1a48-11e9-ab14-d663bd873d93";

pub const SPECIAL_BLE_RX_UUIDS: &[&str] = &[
    "00001000-0000-1000-8992-00805f9b34fb",
    "48090001-1a48-11e9-ab14-d663bd873d93",
];
pub const SPECIAL_BLE_TX_UUIDS: &[&str] = &[
    "00001000-0000-1000-8993-00805f9b34fb",
    "48090002-1a48-11e9-ab14-d663bd873d93",
];
pub const W800K_SERVICE_UUID: &str = "00001000-0000-1000-8991-00805f9b34fb";
