use launcher_game::{Level, LevelTracker};

#[test]
fn levels_follow_log4j_headers_and_carry_over() {
    let mut tracker = LevelTracker::default();
    let lines = [
        (
            "[12:00:01] [main/INFO] (FabricLoader) Loading 3 mods",
            Level::Info,
        ),
        (
            "[12:00:02] [Render thread/WARN]: Missing texture",
            Level::Warn,
        ),
        ("[12:00:03] [Worker-Main-1/ERROR]: Crash", Level::Error),
        ("java.lang.IllegalStateException: boom", Level::Error),
        ("\tat net.minecraft.Foo.bar(Foo.java:1)", Level::Error),
        ("[12:00:04] [main/DEBUG]: done", Level::Debug),
    ];
    for (text, expected) in lines {
        assert_eq!(tracker.line(text.to_owned()).level, expected, "{text}");
    }
}
