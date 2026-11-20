use rikka_nix_edit::{Document, Expr, Formals, Set};

use crate::{
    answers::{Answers, Channel, RIKKA_NIXPKGS},
    catalog::PACKAGES_FLAKE,
    error::Result,
};

const RIKKA_MODULE: &str = "rikka.nixosModules.default";

const SYSTEM: &str = "system";

const HOST_NAME: &str = "system.config.networking.hostName";

pub(crate) fn render(answers: &Answers) -> Result<String> {
    let inputs = Set::new().bind("rikka.url", PACKAGES_FLAKE);
    let inputs = match answers.channel {
        Channel::Unstable => inputs.bind("nixpkgs.follows", RIKKA_NIXPKGS),
        Channel::Stable => inputs.bind(
            "nixpkgs.url",
            format!("github:NixOS/nixpkgs/nixos-{}", answers.release),
        ),
    };

    let system = Expr::apply(
        Expr::reference("nixpkgs.lib.nixosSystem")?,
        Set::new()
            .bind("specialArgs", Set::new().inherit(["inputs"]))
            .bind(
                "modules",
                [
                    Expr::reference(RIKKA_MODULE)?,
                    Expr::path("./configuration.nix"),
                ],
            ),
    );
    let configurations = Set::new().bind(
        "nixosConfigurations",
        Set::new().bind_dynamic(Expr::reference(HOST_NAME)?, Expr::reference(SYSTEM)?),
    );
    let outputs = Expr::lambda(
        Formals::new(["nixpkgs", "rikka"])
            .ellipsis()
            .bound("inputs"),
        Expr::let_in(Set::new().bind(SYSTEM, system), configurations),
    );

    let flake = Set::new()
        .bind(
            "description",
            format!("NixOS configuration for {}", answers.hostname),
        )
        .blank()
        .bind("inputs", inputs)
        .blank()
        .bind("outputs", outputs);
    Ok(Document::new(flake)?.to_string())
}
