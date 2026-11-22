//! Small, versioned window-to-window MIDI protocol shared with Virtual Synthphone E.
pub const TYPE: &str = "k2-synthphone";
pub const VERSION: u32 = 1;

/// Only complete, short channel messages cross this bridge. No SysEx, running
/// status, realtime clocks, or arbitrary worker commands are accepted.
pub fn valid_midi(bytes: &[u8]) -> bool {
    let Some(&status) = bytes.first() else {
        return false;
    };
    let len = match status & 0xf0 {
        0x80..=0xb0 | 0xe0 => 3,
        0xc0 | 0xd0 => 2,
        _ => return false,
    };
    bytes.len() == len && bytes[1..].iter().all(|&byte| byte < 128)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permits_notes_controls_bends_and_programs_on_every_channel() {
        for channel in 0..16 {
            for status in [0x80, 0x90, 0xa0, 0xb0, 0xe0] {
                assert!(valid_midi(&[status | channel, 0, 127]));
            }
            for status in [0xc0, 0xd0] {
                assert!(valid_midi(&[status | channel, 127]));
            }
        }
    }
    #[test]
    fn rejects_partial_running_status_sysex_and_out_of_range_data() {
        for bytes in [
            &[][..],
            &[60, 100],
            &[0x90],
            &[0x90, 60],
            &[0xc0, 1, 0],
            &[0x90, 60, 128],
            &[0xf0, 1, 0xf7],
            &[0xf8],
            &[0x90, 60, 1, 0x80, 60, 0],
        ] {
            assert!(!valid_midi(bytes), "{bytes:?}");
        }
    }
}
