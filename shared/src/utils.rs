use bevy::prelude::*;

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "kB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = &UNITS[0];

    for next_unit in &UNITS[1..] {
        if size < 1024.0 {
            break;
        }
        size /= 1024.0;
        unit = next_unit;
    }

    if unit == &"bytes" {
        format!("{} {}", size as u64, unit)
    } else {
        format!("{size:.1} {unit}")
    }
}
