use crate::RgbColor;

pub const NEOPIXEL_DATA_BITS: usize = 24;

#[cfg(target_arch = "xtensa")]
pub const NEOPIXEL_FRAME_LEN: usize = NEOPIXEL_DATA_BITS + 1;
#[cfg(target_arch = "xtensa")]
const WS2812_T0H_TICKS: u16 = 32;
#[cfg(target_arch = "xtensa")]
const WS2812_T0L_TICKS: u16 = 68;
#[cfg(target_arch = "xtensa")]
const WS2812_T1H_TICKS: u16 = 64;
#[cfg(target_arch = "xtensa")]
const WS2812_T1L_TICKS: u16 = 36;

pub fn neopixel_bits(color: RgbColor) -> [bool; NEOPIXEL_DATA_BITS] {
    let mut bits = [false; NEOPIXEL_DATA_BITS];
    let grb = [color.g, color.r, color.b];
    let mut bit_index = 0usize;

    for channel in grb {
        let mut mask = 0x80u8;
        while mask != 0 {
            bits[bit_index] = (channel & mask) != 0;
            bit_index += 1;
            mask >>= 1;
        }
    }

    bits
}

#[cfg(target_arch = "xtensa")]
pub fn neopixel_frame(color: RgbColor) -> [esp_hal::rmt::PulseCode; NEOPIXEL_FRAME_LEN] {
    use esp_hal::{
        gpio::Level,
        rmt::PulseCode,
    };

    let mut frame = [PulseCode::end_marker(); NEOPIXEL_FRAME_LEN];
    let bits = neopixel_bits(color);

    for (idx, bit_is_set) in bits.iter().enumerate() {
        frame[idx] = if *bit_is_set {
            PulseCode::new(
                Level::High,
                WS2812_T1H_TICKS,
                Level::Low,
                WS2812_T1L_TICKS,
            )
        } else {
            PulseCode::new(
                Level::High,
                WS2812_T0H_TICKS,
                Level::Low,
                WS2812_T0L_TICKS,
            )
        };
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitstream_has_24_entries() {
        let bits = neopixel_bits(RgbColor::new(0, 0, 0));
        assert_eq!(bits.len(), NEOPIXEL_DATA_BITS);
    }

    #[test]
    fn grb_order_is_used_for_color_bits() {
        let bits = neopixel_bits(RgbColor::new(0x00, 0xFF, 0x00));
        assert!(bits[..8].iter().all(|bit| *bit));
        assert!(bits[8..24].iter().all(|bit| !*bit));
    }

    #[test]
    fn red_channel_maps_to_middle_byte_in_grb_stream() {
        let bits = neopixel_bits(RgbColor::new(0x80, 0x00, 0x00));
        assert!(bits[8]);
        assert!(bits[9..16].iter().all(|bit| !*bit));
    }
}