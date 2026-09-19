use crate::constants::{BLE_CHUNK_LEN, CHECKSUM_INIT, RX_HEAD_BB, RX_HEAD_CC, TX_HEAD};
use crate::error::ProtocolError;

/// 已去掉校验和的入站帧.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingFrame {
    pub head: u8,
    pub body: Vec<u8>,
}

impl IncomingFrame {
    pub fn is_response(&self) -> bool {
        self.head == RX_HEAD_BB
    }

    pub fn is_ack(&self) -> bool {
        self.head == RX_HEAD_CC
    }
}

pub fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = CHECKSUM_INIT;
    for b in bytes {
        sum = sum.wrapping_add(*b as u16);
    }
    sum
}

pub fn append_checksum(frame: &mut Vec<u8>) {
    let sum = checksum(frame);
    frame.push((sum >> 8) as u8);
    frame.push((sum & 0xFF) as u8);
}

pub fn verify_checksum(frame: &[u8]) -> Result<(), ProtocolError> {
    if frame.len() < 2 {
        return Err(ProtocolError::Checksum {
            expected: 0,
            actual: 0,
        });
    }
    let (body, tail) = frame.split_at(frame.len() - 2);
    let expected = checksum(body);
    let actual = u16::from_be_bytes([tail[0], tail[1]]);
    if expected == actual {
        Ok(())
    } else {
        Err(ProtocolError::Checksum { expected, actual })
    }
}

/// 把命令载荷封装成 AA 发送帧.
pub fn encode_tx(body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if body.len() > 255 {
        return Err(ProtocolError::PayloadTooLong);
    }
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.push(TX_HEAD);
    frame.push(body.len() as u8);
    frame.extend_from_slice(body);
    append_checksum(&mut frame);
    Ok(frame)
}

/// 按 BLE MTU 切开完整帧.
pub fn split_ble_chunks(frame: &[u8]) -> Vec<Vec<u8>> {
    frame.chunks(BLE_CHUNK_LEN).map(|c| c.to_vec()).collect()
}

pub fn parse_hex(input: &str) -> Result<Vec<u8>, ProtocolError> {
    let filtered: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if !filtered.len().is_multiple_of(2) {
        return Err(ProtocolError::OddHexLength);
    }
    let mut out = Vec::with_capacity(filtered.len() / 2);
    let bytes = filtered.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

pub fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(TABLE[(b >> 4) as usize] as char);
        s.push(TABLE[(b & 0x0F) as usize] as char);
    }
    s
}

fn hex_val(c: u8) -> Result<u8, ProtocolError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(ProtocolError::InvalidHex),
    }
}

enum Extract {
    NeedMore,
    Frame(IncomingFrame),
    Err(ProtocolError),
}

/// 粘包缓冲, 从字节流切出 BB/CC 帧.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }

    pub fn push(&mut self, data: &[u8]) -> Vec<Result<IncomingFrame, ProtocolError>> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            match try_extract(&mut self.buf) {
                Extract::NeedMore => break,
                Extract::Frame(frame) => out.push(Ok(frame)),
                Extract::Err(err) => out.push(Err(err)),
            }
        }
        out
    }
}

fn try_extract(buf: &mut Vec<u8>) -> Extract {
    if buf.is_empty() {
        return Extract::NeedMore;
    }
    let head = buf[0];
    if head != RX_HEAD_BB && head != RX_HEAD_CC {
        buf.remove(0);
        return Extract::Err(ProtocolError::UnexpectedHead(head));
    }
    if buf.len() < 2 {
        return Extract::NeedMore;
    }
    let total = buf[1] as usize + 4;
    if buf.len() < total {
        return Extract::NeedMore;
    }
    let frame: Vec<u8> = buf.drain(..total).collect();
    match verify_checksum(&frame) {
        Ok(()) => Extract::Frame(IncomingFrame {
            head,
            body: frame[2..frame.len() - 2].to_vec(),
        }),
        Err(err) => Extract::Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_noise_normal_matches_capture() {
        let frame = encode_tx(&[0xC1, 0x01]).unwrap();
        assert_eq!(to_hex(&frame), "AA02C1012187");
    }

    #[test]
    fn encode_disconnect_matches_capture() {
        let frame = encode_tx(&[0xCD]).unwrap();
        assert_eq!(to_hex(&frame), "AA01CD2191");
    }

    #[test]
    fn encode_name_utf8_matches_capture() {
        let mut body = vec![0xCA];
        body.extend_from_slice("中文测试".as_bytes());
        let frame = encode_tx(&body).unwrap();
        assert_eq!(to_hex(&frame), "AA0DCAE4B8ADE69687E6B58BE8AF952A38");
    }

    #[test]
    fn decode_battery_response() {
        let raw = parse_hex("BB02D04D21F3").unwrap();
        let mut dec = FrameDecoder::new();
        let frames = dec.push(&raw);
        assert_eq!(frames.len(), 1);
        let frame = frames[0].as_ref().unwrap();
        assert_eq!(frame.head, RX_HEAD_BB);
        assert_eq!(frame.body, vec![0xD0, 0x4D]);
    }

    #[test]
    fn decode_split_across_pushes() {
        let raw = parse_hex("BB02D04D21F3").unwrap();
        let mut dec = FrameDecoder::new();
        assert!(dec.push(&raw[..3]).is_empty());
        let frames = dec.push(&raw[3..]);
        assert_eq!(frames[0].as_ref().unwrap().body, vec![0xD0, 0x4D]);
    }

    #[test]
    fn skip_unexpected_head() {
        let mut dec = FrameDecoder::new();
        let mut data = vec![0x00];
        data.extend(parse_hex("BB02D04D21F3").unwrap());
        let frames = dec.push(&data);
        assert!(matches!(frames[0], Err(ProtocolError::UnexpectedHead(0x00))));
        assert_eq!(frames[1].as_ref().unwrap().body, vec![0xD0, 0x4D]);
    }

    #[test]
    fn ble_chunks_split_at_20() {
        let frame = vec![0u8; 25];
        let chunks = split_ble_chunks(&frame);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 20);
        assert_eq!(chunks[1].len(), 5);
    }
}
