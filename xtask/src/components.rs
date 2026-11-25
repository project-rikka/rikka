use crate::shipped::{
    APPLICATIONS_DIRECTORY, About, Completion, Component, Contents, DataFile, Library, Offer,
    PORTALS, Program, SymbolicIcon, data_file, icon, named, package,
};

const MIME_PACKAGES: &str = "share/mime/packages";

const DBUS_SERVICES: &str = "share/dbus-1/services";

const USER_UNITS: &str = "share/systemd/user";

const SYSTEM_BUS_POLICIES: &str = "share/dbus-1/system.d";

const SYSTEM_BUS_SERVICES: &str = "share/dbus-1/system-services";

const SYSTEM_UNITS: &str = "lib/systemd/system";

pub(crate) const COMPONENTS: &[Component] = &[
    BASE,
    FILES,
    SETTINGS,
    UPDATES,
    TERMINAL,
    application(
        "browser",
        Offer::Selected,
        &[named(
            "//apps/system/browser:rikka-browser",
            "rikka-browser",
        )],
        &[data_file(
            "apps/system/browser/rikka-browser.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-browser", "explore")],
    ),
    application(
        "viewer",
        Offer::Selected,
        &[named("//apps/system/viewer:rikka-viewer", "rikka-viewer")],
        &[
            data_file(
                "apps/system/viewer/rikka-viewer.desktop",
                APPLICATIONS_DIRECTORY,
            ),
            data_file("apps/system/viewer/rikka-viewer.xml", MIME_PACKAGES),
        ],
        &[icon("rikka-viewer", "image")],
    ),
    application(
        "calculator",
        Offer::Selected,
        &[named(
            "//apps/system/calculator:rikka-calculator",
            "rikka-calculator",
        )],
        &[data_file(
            "apps/system/calculator/rikka-calculator.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-calculator", "calculate")],
    ),
    application(
        "task-manager",
        Offer::Selected,
        &[named(
            "//apps/system/task-manager:rikka-task-manager",
            "rikka-task-manager",
        )],
        &[data_file(
            "apps/system/task-manager/rikka-task-manager.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-task-manager", "monitoring")],
    ),
    application(
        "packages",
        Offer::Selected,
        &[named(
            "//apps/system/packages:rikka-packages",
            "rikka-packages",
        )],
        &[data_file(
            "apps/system/packages/rikka-packages.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-packages", "package_2")],
    ),
    application(
        "cut",
        Offer::Selected,
        &[named("//apps/general/cut:rikka-cut", "rikka-cut")],
        &[data_file(
            "apps/general/cut/rikka-cut.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-cut", "movie_edit")],
    )
    .on_windows("//apps/general/cut:rikka-cut_windows_zip"),
    application(
        "music",
        Offer::Selected,
        &[named("//apps/system/music:rikka-music", "rikka-music")],
        &[data_file(
            "apps/system/music/rikka-music.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-music", "music_note")],
    ),
    application(
        "paint",
        Offer::Selected,
        &[named("//apps/general/paint:rikka-paint", "rikka-paint")],
        &[data_file(
            "apps/general/paint/rikka-paint.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-paint", "brush")],
    )
    .on_windows("//apps/general/paint:rikka-paint_windows_zip"),
    application(
        "ade",
        Offer::Offered,
        &[named("//apps/system/ade:rikka-ade", "rikka-ade")],
        &[data_file(
            "apps/system/ade/rikka-ade.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-ade", "code")],
    ),
    application(
        "animate",
        Offer::Offered,
        &[named(
            "//apps/general/animate:rikka-animate",
            "rikka-animate",
        )],
        &[
            data_file(
                "apps/general/animate/rikka-animate.desktop",
                APPLICATIONS_DIRECTORY,
            ),
            data_file("apps/general/animate/rikka-animate.xml", MIME_PACKAGES),
        ],
        &[icon("rikka-animate", "sprint")],
    )
    .on_windows("//apps/general/animate:rikka-animate_windows_zip"),
    application(
        "discord",
        Offer::Offered,
        &[named(
            "//apps/system/discord:rikka-discord",
            "rikka-discord",
        )],
        &[data_file(
            "apps/system/discord/rikka-discord.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-discord", "forum")],
    ),
    application(
        "proton",
        Offer::Offered,
        &[named("//apps/system/proton:rikka-proton", "rikka-proton")],
        &[data_file(
            "apps/system/proton/rikka-proton.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-proton", "shield_lock")],
    ),
    application(
        "remote-viewer",
        Offer::Offered,
        &[named(
            "//apps/general/remote:rikka-remote-viewer",
            "rikka-remote-viewer",
        )],
        &[data_file(
            "apps/general/remote/rikka-remote-viewer.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-remote-viewer", "connected_tv")],
    )
    .on_windows("//apps/general/remote:rikka-remote-viewer_windows_zip"),
    application(
        "launcher",
        Offer::Offered,
        &[named(
            "//apps/general/launcher:rikka-launcher",
            "rikka-launcher",
        )],
        &[data_file(
            "apps/general/launcher/rikka-launcher.desktop",
            APPLICATIONS_DIRECTORY,
        )],
        &[icon("rikka-launcher", "sports_esports")],
    )
    .on_windows("//apps/general/launcher:rikka-launcher_windows_zip"),
    REMOTE,
    REPLAY,
    SDK,
];

const fn application(
    id: &'static str,
    offer: Offer,
    programs: &'static [Program],
    data: &'static [DataFile],
    icons: &'static [SymbolicIcon],
) -> Component {
    Component {
        id,
        about: About::Entry,
        offer,
        windows: None,
        pam: &[],
        session: Contents {
            programs,
            data,
            icons,
            ..Contents::EMPTY
        },
        system: Contents::EMPTY,
    }
}

const BASE: Component = Component {
    id: Component::BASE,
    about: About::Text {
        name: "Rikka desktop",
        summary: "The compositor, shell, keyring, screen sharing and system services",
    },
    offer: Offer::Required,
    windows: None,
    pam: &["rikka-lock"],
    session: Contents {
        programs: &[
            named("//system/rikka-cast:rikka-portal-cast", "rikka-portal-cast"),
            named("//system/rikka-cli:rikka-cli", "rikka"),
            named(
                "//system/rikka-compositor:rikka-compositor",
                "rikka-compositor",
            ),
            named("//system/rikka-keyring:rikka-keyring", "rikka-keyring"),
            named("//system/rikka-shell:rikka-shell", "rikka-shell"),
        ],
        data: &[
            data_file("system/rikka-cast/rikka-cast.portal", PORTALS),
            data_file(
                "system/rikka-compositor/mimeapps.list",
                APPLICATIONS_DIRECTORY,
            ),
            data_file(
                "system/rikka-compositor/rikka-xdg-terminals.list",
                "share/xdg-terminal-exec",
            ),
            data_file(
                "system/rikka-compositor/rikka.desktop",
                "share/wayland-sessions",
            ),
            data_file(
                "system/rikka-compositor/rikka-portals.conf",
                "share/xdg-desktop-portal",
            ),
            data_file("system/rikka-compositor/rikka-session.target", USER_UNITS),
            data_file(
                "system/rikka-compositor/rikka-session-shutdown.target",
                USER_UNITS,
            ),
        ],
        templates: &[
            data_file(
                "system/rikka-cast/org.freedesktop.impl.portal.desktop.rikkacast.service.in",
                DBUS_SERVICES,
            ),
            data_file("system/rikka-cast/rikka-portal-cast.service.in", USER_UNITS),
            data_file(
                "system/rikka-keyring/org.freedesktop.secrets.service.in",
                DBUS_SERVICES,
            ),
            data_file("system/rikka-keyring/rikka-keyring.service.in", USER_UNITS),
        ],
        completions: &[
            Completion {
                target: "//system/rikka-cli/crates/cli-completions:bash",
                into: "share/bash-completion/completions",
            },
            Completion {
                target: "//system/rikka-cli/crates/cli-completions:fish",
                into: "share/fish/vendor_completions.d",
            },
            Completion {
                target: "//system/rikka-cli/crates/cli-completions:zsh",
                into: "share/zsh/site-functions",
            },
        ],
        ..Contents::EMPTY
    },
    system: Contents {
        programs: &[
            named("//system/rikka-boost:rikka-boost", "rikka-boost"),
            named("//system/rikka-nixctl:rikka-nixctl", "rikka-nixctl"),
            named(
                "//system/rikka-pam-helper:rikka-pam-helper",
                "rikka-pam-helper",
            ),
        ],
        data: &[
            data_file(
                "system/rikka-boost/os.rikka.Boost1.conf",
                SYSTEM_BUS_POLICIES,
            ),
            data_file(
                "system/rikka-nixctl/os.rikka.Nix1.conf",
                SYSTEM_BUS_POLICIES,
            ),
            data_file(
                "system/rikka-nixctl/os.rikka.nix.policy",
                "share/polkit-1/actions",
            ),
        ],
        templates: &[
            data_file(
                "system/rikka-boost/os.rikka.Boost1.service.in",
                SYSTEM_BUS_SERVICES,
            ),
            data_file("system/rikka-boost/rikka-boost.service.in", SYSTEM_UNITS),
            data_file(
                "system/rikka-nixctl/os.rikka.Nix1.service.in",
                SYSTEM_BUS_SERVICES,
            ),
            data_file("system/rikka-nixctl/rikka-nixctl.service.in", SYSTEM_UNITS),
        ],
        ..Contents::EMPTY
    },
};

const FILES: Component = Component {
    id: "files",
    about: About::Entry,
    offer: Offer::Required,
    windows: None,
    pam: &[],
    session: Contents {
        programs: &[
            named("//apps/system/files:rikka-files", "rikka-files"),
            named("//apps/system/files:rikka-file-dialog", "rikka-file-dialog"),
            named(
                "//apps/system/files/crates/files-opener:rikka-open",
                "rikka-open",
            ),
            named(
                "//apps/system/files/crates/files-portal:rikka-portal-files",
                "rikka-portal-files",
            ),
            named(
                "//apps/system/files/crates/files-video-thumbnailer:rikka-video-thumbnailer",
                "rikka-video-thumbnailer",
            ),
        ],
        data: &[
            data_file(
                "apps/system/files/rikka-files.desktop",
                APPLICATIONS_DIRECTORY,
            ),
            data_file(
                "apps/system/files/crates/files-portal/rikka-files.portal",
                PORTALS,
            ),
        ],
        templates: &[
            data_file(
                "apps/system/files/crates/files-portal/org.freedesktop.impl.portal.desktop.rikkafiles.service.in",
                DBUS_SERVICES,
            ),
            data_file(
                "apps/system/files/crates/files-portal/rikka-portal-files.service.in",
                USER_UNITS,
            ),
            data_file(
                "apps/system/files/crates/files-video-thumbnailer/rikka-video.thumbnailer.in",
                "share/thumbnailers",
            ),
        ],
        icons: &[icon("rikka-files", "folder")],
        ..Contents::EMPTY
    },
    system: Contents::EMPTY,
};

const SETTINGS: Component = application(
    "settings",
    Offer::Required,
    &[named(
        "//apps/system/settings:rikka-settings",
        "rikka-settings",
    )],
    &[data_file(
        "apps/system/settings/rikka-settings.desktop",
        APPLICATIONS_DIRECTORY,
    )],
    &[icon("rikka-settings", "settings")],
);

const UPDATES: Component = application(
    "updates",
    Offer::Required,
    &[named(
        "//apps/system/updates:rikka-updates",
        "rikka-updates",
    )],
    &[data_file(
        "apps/system/updates/rikka-updates.desktop",
        APPLICATIONS_DIRECTORY,
    )],
    &[icon("rikka-updates", "system_update")],
);

const TERMINAL: Component = application(
    "terminal",
    Offer::Required,
    &[named(
        "//apps/system/terminal:rikka-terminal",
        "rikka-terminal",
    )],
    &[data_file(
        "apps/system/terminal/rikka-terminal.desktop",
        APPLICATIONS_DIRECTORY,
    )],
    &[icon("rikka-terminal", "terminal")],
);

const REMOTE: Component = Component {
    id: "remote",
    about: About::Text {
        name: "Remote desktop server",
        summary: "Serves this session to Rikka Remote Viewer over QUIC",
    },
    offer: Offer::Offered,
    windows: None,
    pam: &["rikka-remote"],
    session: Contents {
        programs: &[named("//system/rikka-remote:rikka-remote", "rikka-remote")],
        templates: &[data_file(
            "system/rikka-remote/rikka-remote.service.in",
            USER_UNITS,
        )],
        ..Contents::EMPTY
    },
    system: Contents::EMPTY,
};

const REPLAY: Component = Component {
    id: "replay",
    about: About::Text {
        name: "Instant replay",
        summary: "Keeps the last minutes of the screen in memory to save as a clip",
    },
    offer: Offer::Offered,
    windows: None,
    pam: &[],
    session: Contents {
        programs: &[named("//system/rikka-replay:rikka-replay", "rikka-replay")],
        templates: &[data_file(
            "system/rikka-replay/rikka-replay.service.in",
            USER_UNITS,
        )],
        ..Contents::EMPTY
    },
    system: Contents::EMPTY,
};

const SDK: Component = Component {
    id: "sdk",
    about: About::Text {
        name: "Application SDK",
        summary: "librikka with C and C++ headers and Go, C#, Java and Python bindings",
    },
    offer: Offer::Offered,
    windows: None,
    pam: &[],
    session: Contents {
        data: &[
            data_file("sdk/bindings/c/include/rikka/rikka.h", "include/rikka"),
            data_file("sdk/bindings/cpp/include/rikka/rikka.hpp", "include/rikka"),
        ],
        templates: &[data_file("sdk/bindings/c/rikka.pc.in", "lib/pkgconfig")],
        libraries: &[Library {
            target: "//sdk/native:versioned",
            link: "librikka.so",
        }],
        packages: &[
            package("//sdk/bindings/csharp:package", "share/rikka/sdk/csharp"),
            package("//sdk/bindings/go:package", "share/rikka/sdk/go"),
            package("//sdk/bindings/java:package", "share/rikka/sdk/java"),
            package("//sdk/bindings/python:package", "share/rikka/sdk/python"),
        ],
        ..Contents::EMPTY
    },
    system: Contents::EMPTY,
};
