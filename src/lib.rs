#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_arch = "xtensa", feature(impl_trait_in_assoc_type))]
extern crate alloc;

pub mod neopixel;
#[cfg(target_arch = "xtensa")]
pub mod web;
#[cfg(target_arch = "xtensa")]
pub mod wifi;

/// Initialise a static variable and return a reference to it.
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$t> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

#[cfg(target_arch = "xtensa")]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

/// RGB color value used by the NeoPixel control path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// Construct an RGB color and apply a brightness scale in one step.
    pub const fn rgb_with_brightness(r: u8, g: u8, b: u8, brightness: u8) -> Self {
        Self::new(r, g, b).with_brightness(brightness)
    }

    /// Construct a full-scale RGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Scale all channels by brightness in the range 0..=255.
    pub const fn with_brightness(self, brightness: u8) -> Self {
        Self {
            r: scale_channel(self.r, brightness),
            g: scale_channel(self.g, brightness),
            b: scale_channel(self.b, brightness),
        }
    }

    /// Shortcut for blue with a brightness scale.
    pub const fn blue(brightness: u8) -> Self {
        Self::rgb_with_brightness(0, 0, 255, brightness)
    }

    /// Shortcut for green with a brightness scale.
    pub const fn green(brightness: u8) -> Self {
        Self::rgb_with_brightness(0, 255, 0, brightness)
    }

    /// Shortcut for red with a brightness scale.
    pub const fn red(brightness: u8) -> Self {
        Self::rgb_with_brightness(255, 0, 0, brightness)
    }

    /// Shortcut for yellow with a brightness scale.
    pub const fn yellow(brightness: u8) -> Self {
        Self::rgb_with_brightness(255, 255, 0, brightness)
    }

    /// Shortcut for orange with brightness scale.
    pub const fn orange(brightness: u8) -> Self {
        Self::rgb_with_brightness(255, 165, 0, brightness)
    }
}

const fn scale_channel(channel: u8, brightness: u8) -> u8 {
    ((channel as u16 * brightness as u16) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::RgbColor;

    #[test]
    fn rgb_with_brightness_zero_turns_off_all_channels() {
        let color = RgbColor::rgb_with_brightness(120, 200, 255, 0);
        assert_eq!(color, RgbColor::new(0, 0, 0));
    }

    #[test]
    fn rgb_with_brightness_full_keeps_original_channels() {
        let color = RgbColor::rgb_with_brightness(12, 34, 56, 255);
        assert_eq!(color, RgbColor::new(12, 34, 56));
    }

    #[test]
    fn with_brightness_scales_each_channel_consistently() {
        let color = RgbColor::new(255, 128, 64).with_brightness(128);
        assert_eq!(color, RgbColor::new(128, 64, 32));
    }

    #[test]
    fn named_color_shortcuts_match_expected_channels() {
        assert_eq!(RgbColor::blue(255), RgbColor::new(0, 0, 255));
        assert_eq!(RgbColor::green(255), RgbColor::new(0, 255, 0));
        assert_eq!(RgbColor::red(255), RgbColor::new(255, 0, 0));
        assert_eq!(RgbColor::yellow(255), RgbColor::new(255, 255, 0));
        assert_eq!(RgbColor::orange(255), RgbColor::new(255, 165, 0));
    }

    #[test]
    fn named_color_shortcuts_apply_brightness() {
        assert_eq!(RgbColor::blue(64), RgbColor::new(0, 0, 64));
        assert_eq!(RgbColor::green(64), RgbColor::new(0, 64, 0));
        assert_eq!(RgbColor::red(64), RgbColor::new(64, 0, 0));
    }
}

/// Shared servo configuration, updated via the web UI.
pub struct LedConfig {
    pub motor_spare_throttle_percent: u16,
    pub motor_left_wheel_throttle_percent: u16,
    pub motor_right_wheel_throttle_percent: u16,
    pub motor_fan_throttle_percent: u16,
    pub motor_fan_idle_throttle_percent: u16,
    pub arm_wait_ms: u64,
    pub on_duration_ms: u64,
    pub on_delay_ms: u64,
}

impl Default for LedConfig {
    fn default() -> Self {
        Self {
            motor_spare_throttle_percent: 0,
            motor_left_wheel_throttle_percent: 50,
            motor_right_wheel_throttle_percent: 50,
            motor_fan_throttle_percent: 100,
            motor_fan_idle_throttle_percent: 20,
            arm_wait_ms: 5000,
            on_duration_ms: 1500,
            on_delay_ms: 0,
        }
    }
}

/// Type alias for the shared config mutex.
#[cfg(target_arch = "xtensa")]
pub type LedConfigMutex = Mutex<CriticalSectionRawMutex, LedConfig>;
