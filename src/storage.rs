use postcard::{from_bytes, to_slice};

use embedded_storage::{ReadStorage, Storage};

use crate::{current_firmware_identity, LedConfig};

pub type FlashMutex = embassy_sync::mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    esp_storage::FlashStorage<'static>,
>;

const CONFIG_MAGIC: [u8; 4] = *b"RWCF";
const CONFIG_VERSION: u8 = 3;
const CONFIG_OFFSET: u32 = 0x9000;
const CONFIG_SECTOR_SIZE: usize = 4096;
const HEADER_SIZE: usize = 20;
const PAYLOAD_MAX_SIZE: usize = CONFIG_SECTOR_SIZE - HEADER_SIZE;

#[derive(Debug)]
pub enum ConfigStorageError {
    Storage(esp_storage::FlashStorageError),
    Encode(postcard::Error),
    Decode(postcard::Error),
}

impl From<esp_storage::FlashStorageError> for ConfigStorageError {
    fn from(value: esp_storage::FlashStorageError) -> Self {
        Self::Storage(value)
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |accumulator, byte| accumulator.wrapping_add(*byte as u32))
}

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn write_u32_le(bytes: &mut [u8], value: u32) {
    bytes.copy_from_slice(&value.to_le_bytes());
}

pub async fn load_led_config(flash: &FlashMutex) -> Result<Option<LedConfig>, ConfigStorageError> {
    let mut sector = [0u8; CONFIG_SECTOR_SIZE];

    {
        let mut flash = flash.lock().await;
        flash.read(CONFIG_OFFSET, &mut sector)?;
    }

    if sector[..4] != CONFIG_MAGIC || sector[4] != CONFIG_VERSION {
        return Ok(None);
    }

    let identity_len = read_u32_le(&sector[8..12]) as usize;
    let payload_len = read_u32_le(&sector[12..16]) as usize;
    let payload_checksum = read_u32_le(&sector[16..20]);

    if identity_len == 0 || payload_len == 0 || identity_len + payload_len > PAYLOAD_MAX_SIZE {
        return Ok(None);
    }

    let identity = &sector[HEADER_SIZE..HEADER_SIZE + identity_len];
    let mut expected_identity_buf = [0u8; PAYLOAD_MAX_SIZE];
    let expected_identity = to_slice(&current_firmware_identity(), &mut expected_identity_buf)
        .map_err(ConfigStorageError::Encode)?;

    if identity != expected_identity {
        return Ok(None);
    }

    let payload = &sector[HEADER_SIZE + identity_len..HEADER_SIZE + identity_len + payload_len];
    if checksum(payload) != payload_checksum {
        return Ok(None);
    }

    let config = from_bytes::<LedConfig>(payload).map_err(ConfigStorageError::Decode)?;
    Ok(Some(config))
}

pub async fn save_led_config(
    flash: &FlashMutex,
    config: &LedConfig,
) -> Result<(), ConfigStorageError> {
    let mut sector = [0u8; CONFIG_SECTOR_SIZE];
    let identity = current_firmware_identity();
    let mut identity_buf = [0u8; PAYLOAD_MAX_SIZE];

    sector[..4].copy_from_slice(&CONFIG_MAGIC);
    sector[4] = CONFIG_VERSION;

    let identity_bytes = to_slice(&identity, &mut identity_buf).map_err(ConfigStorageError::Encode)?;
    let identity_len = identity_bytes.len();

    sector[HEADER_SIZE..HEADER_SIZE + identity_len].copy_from_slice(identity_bytes);

    let payload = to_slice(
        config,
        &mut sector[HEADER_SIZE + identity_len..HEADER_SIZE + PAYLOAD_MAX_SIZE],
    )
    .map_err(ConfigStorageError::Encode)?;
    let payload_len = payload.len();
    let payload_checksum = checksum(payload);

    write_u32_le(&mut sector[8..12], identity_len as u32);
    write_u32_le(&mut sector[12..16], payload_len as u32);
    write_u32_le(&mut sector[16..20], payload_checksum);

    {
        let mut flash = flash.lock().await;
        flash.write(CONFIG_OFFSET, &sector)?;
    }

    Ok(())
}