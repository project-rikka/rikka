use rikka_sound::{Node, Volume};

pub(super) fn percent(volume: &Volume) -> String {
    format!("{}%", (volume.amplitude() * 100.0).round())
}

pub(super) fn decibels(volume: &Volume) -> String {
    match volume.decibels() {
        Some(decibels) => format!("{decibels:+.1} dB"),
        None => "-\u{221e} dB".to_owned(),
    }
}

pub(super) fn balance(balance: f32) -> String {
    let away = (balance.abs() * 100.0).round();
    match balance {
        _ if away == 0.0 => "L \u{2014} R".to_owned(),
        _ if balance < 0.0 => format!("L {away}%"),
        _ => format!("R {away}%"),
    }
}

pub(super) fn hertz(rate: u32) -> String {
    format!("{:.1} kHz", f64::from(rate) / 1000.0)
}

pub(super) fn milliseconds(milliseconds: f64) -> String {
    format!("{milliseconds:.2} ms")
}

pub(super) fn format_summary(node: &Node) -> Option<String> {
    let format = node.format?;
    let mut line = format!(
        "{} \u{b7} {} \u{b7} {}ch \u{b7} {} kbps",
        format.sample.name(),
        hertz(format.rate),
        format.channels,
        format.bitrate() / 1000,
    );
    if let Some(latency) = node.latency() {
        line.push_str(" \u{b7} ");
        line.push_str(&milliseconds(latency));
    }
    Some(line)
}
