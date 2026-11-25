use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use launcher_instance::{GameMode, Instances};
use launcher_nbt::{Compound, Tag};

#[test]
fn worlds_read_level_data() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let instance = instances.create("Worlds").expect("instance");
    for (folder, name, played, mode) in [("old", "Old Days", 100, 0), ("new", "Fresh", 900, 1)] {
        let world = instance.path.join("saves").join(folder);
        std::fs::create_dir_all(&world).expect("world folder");
        let mut version = Compound::new();
        version.insert("Name", Tag::String("26.3".to_owned()));
        let mut data = Compound::new();
        data.insert("LevelName", Tag::String(name.to_owned()));
        data.insert("LastPlayed", Tag::Long(played));
        data.insert("GameType", Tag::Int(mode));
        data.insert("hardcore", Tag::Byte(0));
        data.insert("Version", Tag::Compound(version));
        let mut level = Compound::new();
        level.insert("Data", Tag::Compound(data));
        let bytes = launcher_nbt::write("", &level).expect("nbt");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&bytes).expect("compresses");
        std::fs::write(world.join("level.dat"), encoder.finish().expect("finishes"))
            .expect("level.dat");
    }
    std::fs::create_dir_all(instance.path.join("saves/not-a-world")).expect("stray folder");
    std::fs::create_dir_all(instance.path.join("saves/broken")).expect("broken folder");
    std::fs::write(instance.path.join("saves/broken/level.dat"), "not gzip").expect("level.dat");

    let listed = instance.worlds().expect("worlds list");
    assert_eq!(listed.problems.len(), 1);
    let worlds = listed.worlds;
    assert_eq!(worlds.len(), 2);
    assert_eq!(worlds[0].folder, "new");
    assert_eq!(worlds[0].name, "Fresh");
    assert_eq!(worlds[0].mode, GameMode::Creative);
    assert_eq!(worlds[0].version.as_deref(), Some("26.3"));
    assert_eq!(worlds[1].last_played, Some(100));

    instance.remove_world("old").expect("world removed");
    assert_eq!(instance.worlds().expect("worlds list").worlds.len(), 1);
}

#[test]
fn servers_round_trip_through_servers_dat() {
    let root = tempfile::tempdir().expect("temporary folder");
    let instances = Instances::new(root.path());
    let instance = instances.create("Servers").expect("instance");
    assert!(instance.servers().expect("no servers yet").is_empty());
    instance
        .add_server("Hypixel", "mc.hypixel.net")
        .expect("server added");
    instance
        .add_server("Local", "localhost:25566")
        .expect("server added");
    let servers = instance.servers().expect("servers list");
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[1].name, "Local");
    assert_eq!(servers[1].address, "localhost:25566");
    instance
        .remove_server("mc.hypixel.net")
        .expect("server removed");
    let servers = instance.servers().expect("servers list");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "Local");
}
