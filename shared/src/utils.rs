use std::time::{SystemTime, UNIX_EPOCH};

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

/// Milliseconds since the Unix epoch, 0 if the system clock is set before it.
/// The snake case of a PascalCase name, as the commands and the content files name a block or
/// an item: `OakLog` is `oak_log`.
pub fn snake_case(pascal: &str) -> String {
    let mut name = String::new();
    for c in pascal.chars() {
        if c.is_uppercase() && !name.is_empty() {
            name.push('_');
        }
        name.extend(c.to_lowercase());
    }
    name
}

pub fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since_epoch| since_epoch.as_millis() as u64)
}
