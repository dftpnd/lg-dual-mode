mod ddc;
mod edid;

use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

use edid::Mode;

const DUAL_MODE_OPCODE: u8 = 0xb1;
const GROUP_OFF: u16 = 0x2200;
const INDEX_FULL_WIDE: u16 = 0;
const INDEX_SIXTEEN_NINE: u16 = 1;
const APPLY_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(400);

const USAGE: &str = "\
lg-dual-mode — переключение Dual-Mode монитора LG UltraGear 45GX950A

    lg-dual-mode status     текущий режим
    lg-dual-mode wide       полная ширина, 5120x2160@165
    lg-dual-mode 16:9       16:9 37\", 3840x2160@165
    lg-dual-mode toggle     переключить на другой
";

fn main() -> ExitCode {
    let Some(command) = std::env::args().nth(1) else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match run(&command) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("{reason}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: &str) -> Result<String, String> {
    let connector = edid::find_connector()?;
    let current = edid::mode_of(&connector.edid)
        .ok_or_else(|| "не удалось определить текущий режим".to_string())?;

    let wanted = match command {
        "status" => {
            return Ok(format!(
                "{} — {} ({})",
                current.name(),
                current.description(),
                connector.name
            ));
        }
        "wide" => Mode::FullWide,
        "16:9" => Mode::SixteenNine,
        "toggle" => current.other(),
        "-h" | "--help" | "help" => return Ok(USAGE.trim_end().to_string()),
        other => return Err(format!("неизвестная команда: {other}\n\n{USAGE}")),
    };

    if wanted == current {
        return Ok(format!("уже {} — {}", wanted.name(), wanted.description()));
    }

    let bus = ddc::find_bus(&connector.edid)?;
    ddc::send_set_vcp(&bus, DUAL_MODE_OPCODE, value_for(wanted))?;
    await_mode(wanted)?;
    Ok(format!("{} — {}", wanted.name(), wanted.description()))
}

fn value_for(mode: Mode) -> u16 {
    let index = match mode {
        Mode::FullWide => INDEX_FULL_WIDE,
        Mode::SixteenNine => INDEX_SIXTEEN_NINE,
    };
    GROUP_OFF | index
}

fn await_mode(wanted: Mode) -> Result<(), String> {
    let deadline = Instant::now() + APPLY_TIMEOUT;
    while Instant::now() < deadline {
        sleep(POLL_INTERVAL);
        if let Ok(connector) = edid::find_connector() {
            if edid::mode_of(&connector.edid) == Some(wanted) {
                return Ok(());
            }
        }
    }
    Err(format!(
        "команда отправлена, но монитор не перешёл в {} за {} с",
        wanted.name(),
        APPLY_TIMEOUT.as_secs()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_off_group_values_from_the_decompiled_source() {
        assert_eq!(value_for(Mode::FullWide), 0x2200);
        assert_eq!(value_for(Mode::SixteenNine), 0x2201);
    }
}
