use std::fs;
use std::path::{Path, PathBuf};

pub const EDID_BLOCK_LEN: usize = 128;

const LG_MANUFACTURER: [u8; 2] = [0x1e, 0x6d];
const PRODUCT_FULL_WIDE: u16 = 40605;
const PRODUCT_SIXTEEN_NINE: u16 = 40616;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    FullWide,
    SixteenNine,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::FullWide => "wide",
            Mode::SixteenNine => "16:9",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Mode::FullWide => "Полная ширина, 5120x2160",
            Mode::SixteenNine => "16:9 37\", 3840x2160",
        }
    }

    pub fn other(self) -> Mode {
        match self {
            Mode::FullWide => Mode::SixteenNine,
            Mode::SixteenNine => Mode::FullWide,
        }
    }
}

pub struct Connector {
    pub name: String,
    pub edid: Vec<u8>,
}

pub fn product_code(edid: &[u8]) -> Option<u16> {
    if edid.len() < 12 {
        return None;
    }
    Some(u16::from_le_bytes([edid[10], edid[11]]))
}

pub fn is_lg(edid: &[u8]) -> bool {
    edid.len() >= 10 && edid[8..10] == LG_MANUFACTURER
}

pub fn mode_of(edid: &[u8]) -> Option<Mode> {
    match product_code(edid)? {
        PRODUCT_FULL_WIDE => Some(Mode::FullWide),
        PRODUCT_SIXTEEN_NINE => Some(Mode::SixteenNine),
        _ => None,
    }
}

pub fn find_connector() -> Result<Connector, String> {
    let entries = fs::read_dir("/sys/class/drm")
        .map_err(|e| format!("не читается /sys/class/drm: {e}"))?;
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("edid").is_file())
        .collect();
    candidates.sort();

    for path in &candidates {
        if !connector_is_connected(path) {
            continue;
        }
        let Ok(edid) = fs::read(path.join("edid")) else {
            continue;
        };
        if edid.len() >= EDID_BLOCK_LEN && is_lg(&edid) && mode_of(&edid).is_some() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Ok(Connector { name, edid });
        }
    }
    Err("монитор LG с поддержкой Dual-Mode не найден среди подключённых".to_string())
}

fn connector_is_connected(path: &Path) -> bool {
    fs::read_to_string(path.join("status"))
        .map(|s| s.trim() == "connected")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edid_with(manufacturer: [u8; 2], product: u16) -> Vec<u8> {
        let mut edid = vec![0u8; EDID_BLOCK_LEN];
        edid[8..10].copy_from_slice(&manufacturer);
        edid[10..12].copy_from_slice(&product.to_le_bytes());
        edid
    }

    #[test]
    fn reads_product_code_little_endian() {
        let edid = edid_with(LG_MANUFACTURER, 40605);
        assert_eq!(product_code(&edid), Some(40605));
    }

    #[test]
    fn recognises_both_dual_mode_products() {
        assert_eq!(mode_of(&edid_with(LG_MANUFACTURER, 40605)), Some(Mode::FullWide));
        assert_eq!(mode_of(&edid_with(LG_MANUFACTURER, 40616)), Some(Mode::SixteenNine));
    }

    #[test]
    fn rejects_unknown_product() {
        assert_eq!(mode_of(&edid_with(LG_MANUFACTURER, 12345)), None);
    }

    #[test]
    fn detects_lg_manufacturer() {
        assert!(is_lg(&edid_with(LG_MANUFACTURER, 40605)));
        assert!(!is_lg(&edid_with([0x04, 0x72], 40605)));
    }

    #[test]
    fn short_edid_yields_no_product() {
        assert_eq!(product_code(&[0u8; 4]), None);
    }

    #[test]
    fn other_flips_the_mode() {
        assert_eq!(Mode::FullWide.other(), Mode::SixteenNine);
        assert_eq!(Mode::SixteenNine.other(), Mode::FullWide);
    }
}
