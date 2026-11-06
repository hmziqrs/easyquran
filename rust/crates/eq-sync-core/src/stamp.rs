//! Stamps: `{ms, 13 digits}-{counter, 4 hex}-{device id}`. Fixed widths make a
//! bytewise compare the same as comparing the parts in order.

use std::cmp::Ordering;

const DEVICE_MAX: usize = 64;
const COUNTER_MAX: u32 = 0xffff;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStamp {
    pub ms: u64,
    pub counter: u32,
    pub device: String,
}

pub fn parse_stamp(stamp: &str) -> Option<ParsedStamp> {
    let bytes = stamp.as_bytes();
    if bytes.len() < 20 || bytes[13] != b'-' || bytes[18] != b'-' {
        return None;
    }
    let ms_part = &stamp[..13];
    let counter_part = &stamp[14..18];
    let device = &stamp[19..];
    if !ms_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !counter_part
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let device_ok = device.len() <= DEVICE_MAX
        && device
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !device_ok {
        return None;
    }
    Some(ParsedStamp {
        ms: ms_part.parse().ok()?,
        counter: u32::from_str_radix(counter_part, 16).ok()?,
        device: device.to_string(),
    })
}

pub fn is_valid_stamp(stamp: &str) -> bool {
    parse_stamp(stamp).is_some()
}

pub fn format_stamp(ms: u64, counter: u32, device: &str) -> String {
    format!("{ms:013}-{:04x}-{device}", counter.min(COUNTER_MAX))
}

/// `None` (a row written before stamps existed) ranks lowest.
pub fn compare_stamps(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => x.as_bytes().cmp(y.as_bytes()),
    }
}
