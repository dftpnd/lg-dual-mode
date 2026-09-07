use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use crate::edid::EDID_BLOCK_LEN;

const I2C_SLAVE: libc::c_ulong = 0x0703;
const EDID_ADDRESS: u16 = 0x50;
const DDC_ADDRESS: u16 = 0x37;
const DDC_WRITE_ADDRESS: u8 = 0x6e;
const HOST_ADDRESS: u8 = 0x51;
const SET_VCP_REQUEST: u8 = 0x03;
const SET_VCP_PAYLOAD_LEN: u8 = 0x84;
const WRITE_SETTLE: Duration = Duration::from_millis(50);

pub struct Bus {
    pub path: PathBuf,
}

pub fn set_vcp_packet(opcode: u8, value: u16) -> [u8; 7] {
    let [high, low] = value.to_be_bytes();
    let mut packet = [
        HOST_ADDRESS,
        SET_VCP_PAYLOAD_LEN,
        SET_VCP_REQUEST,
        opcode,
        high,
        low,
        0,
    ];
    let mut checksum = DDC_WRITE_ADDRESS;
    for byte in &packet[..6] {
        checksum ^= byte;
    }
    packet[6] = checksum;
    packet
}

pub fn find_bus(connector_edid: &[u8]) -> Result<Bus, String> {
    let mut paths: Vec<PathBuf> = (0..64)
        .map(|n| PathBuf::from(format!("/dev/i2c-{n}")))
        .filter(|p| p.exists())
        .collect();
    paths.sort();

    let wanted = &connector_edid[..EDID_BLOCK_LEN];
    let mut denied = 0;
    for path in paths {
        match read_edid(&path) {
            Ok(edid) if edid == wanted => return Ok(Bus { path }),
            Err(BusError::PermissionDenied) => denied += 1,
            _ => {}
        }
    }
    if denied > 0 {
        return Err(format!(
            "нет доступа к {denied} шинам i2c — добавь себя в группу i2c: sudo usermod -aG i2c $USER"
        ));
    }
    Err("шина i2c этого монитора не найдена".to_string())
}

pub fn send_set_vcp(bus: &Bus, opcode: u8, value: u16) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&bus.path)
        .map_err(|e| format!("{}: {e}", bus.path.display()))?;
    select_address(&file, DDC_ADDRESS)?;
    let packet = set_vcp_packet(opcode, value);
    file.write_all(&packet)
        .map_err(|e| format!("запись в {}: {e}", bus.path.display()))?;
    sleep(WRITE_SETTLE);
    Ok(())
}

enum BusError {
    PermissionDenied,
    Other,
}

fn read_edid(path: &PathBuf) -> Result<Vec<u8>, BusError> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => BusError::PermissionDenied,
            _ => BusError::Other,
        })?;
    select_address(&file, EDID_ADDRESS).map_err(|_| BusError::Other)?;
    file.write_all(&[0u8]).map_err(|_| BusError::Other)?;
    let mut block = vec![0u8; EDID_BLOCK_LEN];
    file.read_exact(&mut block).map_err(|_| BusError::Other)?;
    Ok(block)
}

fn select_address(file: &File, address: u16) -> Result<(), String> {
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), I2C_SLAVE, address as libc::c_ulong) };
    if rc < 0 {
        return Err(format!(
            "не удалось выбрать адрес i2c {address:#x}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_documented_full_wide_packet() {
        let packet = set_vcp_packet(0xb1, 0x2200);
        assert_eq!(&packet[..6], &[0x51, 0x84, 0x03, 0xb1, 0x22, 0x00]);
    }

    #[test]
    fn checksum_is_xor_of_destination_and_body() {
        let packet = set_vcp_packet(0xb1, 0x2201);
        let expected = DDC_WRITE_ADDRESS ^ 0x51 ^ 0x84 ^ 0x03 ^ 0xb1 ^ 0x22 ^ 0x01;
        assert_eq!(packet[6], expected);
    }

    #[test]
    fn whole_packet_xors_to_zero_with_destination() {
        let packet = set_vcp_packet(0xb1, 0x2201);
        let folded = packet.iter().fold(DDC_WRITE_ADDRESS, |acc, b| acc ^ b);
        assert_eq!(folded, 0);
    }

    #[test]
    fn splits_value_big_endian() {
        let packet = set_vcp_packet(0x10, 0x1234);
        assert_eq!(packet[4], 0x12);
        assert_eq!(packet[5], 0x34);
    }
}
