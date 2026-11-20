use rikka_hifi::Hifi;

fn playback_pcms() -> Option<Vec<(u32, u32)>> {
    let listed = std::fs::read_to_string("/proc/asound/pcm").ok()?;
    Some(
        listed
            .lines()
            .filter(|line| line.split(" : ").any(|part| part.starts_with("playback")))
            .filter_map(|line| {
                let (card, device) = line.split_once(':')?.0.split_once('-')?;
                Some((card.parse().ok()?, device.parse().ok()?))
            })
            .collect(),
    )
}

#[test]
fn lists_playback_pcms_while_held() {
    let Some(pcms) = playback_pcms() else {
        eprintln!("this machine has no ALSA: the listing test is skipped");
        return;
    };
    let devices = Hifi::devices().expect("the sound cards are listed");
    for (card, device) in pcms {
        let capabilities = devices
            .iter()
            .find(|listed| listed.card == card && listed.id.device == device)
            .and_then(|listed| listed.capabilities.as_ref());
        assert!(
            capabilities.is_some_and(|capabilities| !capabilities.formats.is_empty()
                && !capabilities.rates.is_empty()
                && !capabilities.channels.is_empty()),
            "card {card} device {device} is missing from {devices:#?}"
        );
    }
}
