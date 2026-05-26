#![no_std]
#![feature(impl_trait_in_assoc_type)]

pub mod web;
pub mod wifi;

/// Initialise a static variable and return a reference to it.
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$t> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

/// Shared LED configuration, updated via the web UI.
pub struct LedConfig {
    pub duty_pct: u8,
    pub on_delay_ms: u64,
}

impl Default for LedConfig {
    fn default() -> Self {
        Self {
            duty_pct: 75,
            on_delay_ms: 1500,
        }
    }
}

/// Type alias for the shared config mutex.
pub type LedConfigMutex = Mutex<CriticalSectionRawMutex, LedConfig>;
