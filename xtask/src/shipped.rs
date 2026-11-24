use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Program {
    pub(crate) target: &'static str,
    pub(crate) installed: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DataFile {
    pub(crate) source: &'static str,
    pub(crate) into: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Supplied {
    pub(crate) variable: &'static str,
    pub(crate) file: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SymbolicIcon {
    pub(crate) application: &'static str,
    pub(crate) symbol: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Completion {
    pub(crate) target: &'static str,
    pub(crate) into: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Library {
    pub(crate) target: &'static str,
    pub(crate) link: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Package {
    pub(crate) target: &'static str,
    pub(crate) into: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Contents {
    pub(crate) programs: &'static [Program],
    pub(crate) data: &'static [DataFile],
    pub(crate) templates: &'static [DataFile],
    pub(crate) icons: &'static [SymbolicIcon],
    pub(crate) completions: &'static [Completion],
    pub(crate) libraries: &'static [Library],
    pub(crate) packages: &'static [Package],
}

impl Contents {
    pub(crate) const EMPTY: Contents = Contents {
        programs: &[],
        data: &[],
        templates: &[],
        icons: &[],
        completions: &[],
        libraries: &[],
        packages: &[],
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Offer {
    Required,
    Selected,
    Offered,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum About {
    Entry,
    Text {
        name: &'static str,
        summary: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Component {
    pub(crate) id: &'static str,
    pub(crate) about: About,
    pub(crate) offer: Offer,
    pub(crate) windows: Option<&'static str>,
    pub(crate) pam: &'static [&'static str],
    pub(crate) session: Contents,
    pub(crate) system: Contents,
}

impl Component {
    pub(crate) const BASE: &'static str = "base";

    pub(crate) fn contents(&self) -> [Contents; 2] {
        [self.session, self.system]
    }

    pub(crate) const fn on_windows(self, zip: &'static str) -> Component {
        Component {
            windows: Some(zip),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Distributed {
    Asset,
    Flake,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shared {
    pub(crate) name: &'static str,
    pub(crate) library: &'static str,
    pub(crate) variable: &'static str,
    pub(crate) found_by: &'static str,
    pub(crate) into: &'static str,
    pub(crate) distributed: Distributed,
}

pub(crate) const SHARED: &[Shared] = &[
    Shared {
        name: "gecko",
        library: "//crates/web/rikka-web",
        variable: "RIKKA_GECKO",
        found_by: "RIKKA_GECKO_PATH",
        into: "lib/rikka-gecko",
        distributed: Distributed::Flake,
    },
    Shared {
        name: "grammars",
        library: "//crates/foundation/rikka-grammars",
        variable: "RIKKA_GRAMMARS",
        found_by: "RIKKA_GRAMMAR_PATH",
        into: "lib/rikka/grammars",
        distributed: Distributed::Asset,
    },
    Shared {
        name: "icon-fonts",
        library: "//crates/graphics/rikka-text",
        variable: ICON_FONTS,
        found_by: "RIKKA_ICON_FONT_PATH",
        into: "share/rikka/fonts",
        distributed: Distributed::Flake,
    },
    Shared {
        name: "mesa",
        library: "//crates/graphics/rikka-gpu-drivers",
        variable: "RIKKA_MESA",
        found_by: "RIKKA_MESA_PATH",
        into: "lib/rikka-mesa",
        distributed: Distributed::Flake,
    },
];

pub(crate) const MATERIAL_LISTING: Supplied = Supplied {
    variable: "RIKKA_ICON_LISTINGS",
    file: "MaterialSymbolsRounded.codepoints",
};

pub(crate) const MATERIAL_FONT: Supplied = Supplied {
    variable: ICON_FONTS,
    file: "MaterialSymbolsRounded.ttf",
};

pub(crate) const SYMBOLIC_ICON_DIRECTORY: &str = "share/icons/hicolor/symbolic/apps";

pub(crate) const APPLICATIONS_DIRECTORY: &str = "share/applications";

pub(crate) const PORTALS: &str = "share/xdg-desktop-portal/portals";

const ICON_FONTS: &str = "RIKKA_ICON_FONTS";

pub(crate) const fn named(target: &'static str, installed: &'static str) -> Program {
    Program { target, installed }
}

pub(crate) const fn data_file(source: &'static str, into: &'static str) -> DataFile {
    DataFile { source, into }
}

pub(crate) const fn package(target: &'static str, into: &'static str) -> Package {
    Package { target, into }
}

pub(crate) const fn icon(application: &'static str, symbol: &'static str) -> SymbolicIcon {
    SymbolicIcon {
        application,
        symbol,
    }
}
