use launcher_meta::GameTarget;

pub(crate) fn rikka() -> GameTarget {
    GameTarget {
        minecraft: env!("RIKKA_MINECRAFT_VERSION").to_owned(),
        fabric_loader: env!("RIKKA_FABRIC_LOADER_VERSION").to_owned(),
    }
}
