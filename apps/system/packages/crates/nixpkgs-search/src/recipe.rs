pub(crate) enum RecipeItem {
    Enable(&'static str, bool),
    Value(&'static str, &'static str, bool),
    Group(&'static str, bool),
}

pub(crate) const RECIPES: &[(&str, &[RecipeItem])] = &[
    (
        "docker",
        &[
            RecipeItem::Enable("virtualisation.docker.enable", true),
            RecipeItem::Group("docker", true),
        ],
    ),
    (
        "podman",
        &[
            RecipeItem::Enable("virtualisation.podman.enable", true),
            RecipeItem::Enable("virtualisation.podman.dockerCompat", false),
        ],
    ),
    (
        "virt-manager",
        &[
            RecipeItem::Enable("virtualisation.libvirtd.enable", true),
            RecipeItem::Group("libvirtd", true),
        ],
    ),
    (
        "virtualbox",
        &[
            RecipeItem::Enable("virtualisation.virtualbox.host.enable", true),
            RecipeItem::Group("vboxusers", true),
        ],
    ),
    (
        "waydroid",
        &[RecipeItem::Enable("virtualisation.waydroid.enable", true)],
    ),
    (
        "wireshark",
        &[
            RecipeItem::Enable("programs.wireshark.enable", true),
            RecipeItem::Group("wireshark", true),
        ],
    ),
    ("wireshark-cli", &[RecipeItem::Group("wireshark", true)]),
    (
        "tailscale",
        &[
            RecipeItem::Enable("services.tailscale.enable", true),
            RecipeItem::Value("services.tailscale.useRoutingFeatures", "client", false),
        ],
    ),
    (
        "mullvad-vpn",
        &[RecipeItem::Enable("services.mullvad-vpn.enable", true)],
    ),
    (
        "flatpak",
        &[
            RecipeItem::Enable("services.flatpak.enable", true),
            RecipeItem::Enable("xdg.portal.enable", true),
        ],
    ),
    (
        "appimage-run",
        &[
            RecipeItem::Enable("programs.appimage.enable", true),
            RecipeItem::Enable("programs.appimage.binfmt", false),
        ],
    ),
    (
        "openrgb",
        &[RecipeItem::Enable("services.hardware.openrgb.enable", true)],
    ),
    (
        "piper",
        &[RecipeItem::Enable("services.ratbagd.enable", true)],
    ),
    (
        "kdePackages.kdeconnect-kde",
        &[RecipeItem::Enable("programs.kdeconnect.enable", true)],
    ),
    (
        "gnupg",
        &[RecipeItem::Enable("programs.gnupg.agent.enable", false)],
    ),
    (
        "obs-studio",
        &[RecipeItem::Enable(
            "programs.obs-studio.enableVirtualCamera",
            false,
        )],
    ),
    (
        "localsend",
        &[RecipeItem::Enable("programs.localsend.openFirewall", false)],
    ),
    (
        "steam",
        &[
            RecipeItem::Enable("programs.steam.remotePlay.openFirewall", false),
            RecipeItem::Enable("programs.steam.gamescopeSession.enable", false),
        ],
    ),
];

pub(crate) fn items_for(attribute: &str) -> &'static [RecipeItem] {
    RECIPES
        .iter()
        .find(|(package, _)| *package == attribute)
        .map_or(&[], |(_, wants)| *wants)
}

pub(crate) fn paths() -> impl Iterator<Item = &'static str> {
    RECIPES
        .iter()
        .flat_map(|(_, wants)| wants.iter())
        .filter_map(|want| match want {
            RecipeItem::Enable(path, _) => Some(*path),
            RecipeItem::Value(path, _, _) => Some(*path),
            RecipeItem::Group(..) => None,
        })
}
