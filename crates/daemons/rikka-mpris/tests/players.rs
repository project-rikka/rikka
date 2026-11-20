use std::time::Duration;

use futures_util::StreamExt;
use rikka_mpris::{
    Abilities, Art, Command, Control, Playback, Player, Players, Publisher, Repeat, State, Status,
    Track,
};
use rikka_test_bus::{PrivateBus, TIMEOUT};
use tokio::runtime::Runtime;

fn song() -> Track {
    Track {
        id: "qobuz:track:1".into(),
        title: "Time".into(),
        artists: vec!["Pink Floyd".into()],
        album: Some("The Dark Side of the Moon".into()),
        album_artists: Vec::new(),
        length: Some(Duration::from_secs(413)),
        art: Some(Art::Web("https://example.org/cover.jpg".into())),
        url: None,
        number: Some(4),
    }
}

fn playing(position: Duration) -> Playback {
    Playback {
        status: Status::Playing,
        track: Some(song()),
        position,
        volume: 0.8,
        shuffle: Some(false),
        repeat: Some(Repeat::None),
        abilities: Abilities::default(),
        ..Playback::default()
    }
}

#[test]
fn published_player_is_tracked_and_controlled() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };

    let (publisher, commands) = Publisher::builder("rikka-music")
        .identity("Music")
        .desktop_entry("rikka-music")
        .raises()
        .serve()
        .expect("bus name should be free on a private bus");
    assert_eq!(publisher.bus_name(), "org.mpris.MediaPlayer2.rikka_music");
    publisher.show(playing(Duration::from_secs(20)));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime should build");
    let players = Players::start();
    let found = wait_for_active(&runtime, &players, |player| player.track.is_some());
    assert_eq!(found.identity, "Music");
    assert_eq!(found.desktop_entry.as_deref(), Some("rikka-music"));
    assert_eq!(found.status, Status::Playing);
    let track = found.track.clone().expect("player should report a track");
    assert_eq!(track.title, "Time");
    assert_eq!(track.artist().as_deref(), Some("Pink Floyd"));
    assert_eq!(track.length, Some(Duration::from_secs(413)));
    assert_eq!(found.volume, Some(0.8));
    assert!(found.raises);
    assert!(
        found.position_now() >= Duration::from_secs(20),
        "the position was read and keeps moving"
    );

    let mut paused = playing(Duration::from_secs(25));
    paused.status = Status::Paused;
    publisher.show(paused.clone());
    wait_for_active(&runtime, &players, |player| player.status == Status::Paused);

    let mut moved = paused;
    moved.position = Duration::from_secs(200);
    publisher.show(moved);
    wait_for_active(&runtime, &players, |player| {
        player.position == Duration::from_secs(200)
    });

    players.control(&found.id, Control::PlayPause);
    players.control(&found.id, Control::SeekTo(Duration::from_secs(30)));
    players.control(&found.id, Control::Volume(0.5));
    let received: Vec<Command> = runtime.block_on(async {
        tokio::time::timeout(TIMEOUT, commands.take(3).collect())
            .await
            .expect("player should receive a command")
    });
    for expected in [
        Command::PlayPause,
        Command::SeekTo(Duration::from_secs(30)),
        Command::Volume(0.5),
    ] {
        assert!(
            received.contains(&expected),
            "{expected:?} was not heard in {received:?}"
        );
    }

    drop(publisher);
    wait_for_state(&runtime, &players, |state| {
        state.players.is_empty().then_some(())
    });
    assert_eq!(players.state().active, None);
}

fn wait_for_active(
    runtime: &Runtime,
    players: &Players,
    ready: impl Fn(&Player) -> bool,
) -> Player {
    wait_for_state(runtime, players, |state| {
        state.active().filter(|player| ready(player)).cloned()
    })
}

fn wait_for_state<T>(
    runtime: &Runtime,
    players: &Players,
    ready: impl Fn(&State) -> Option<T>,
) -> T {
    let mut changes = std::pin::pin!(players.changes());
    if let Some(found) = ready(&players.state()) {
        return found;
    }
    runtime.block_on(async {
        tokio::time::timeout(TIMEOUT, async {
            while let Some(state) = changes.next().await {
                if let Some(found) = ready(&state) {
                    return found;
                }
            }
            panic!("player state stream ended")
        })
        .await
        .expect("player state should change in time")
    })
}
