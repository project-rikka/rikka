use rikka_nix_edit::Set;

use crate::answers::Answers;

pub(crate) fn body(answers: &Answers) -> Set {
    Set::new()
        .bind(
            "nix.settings.experimental-features",
            ["nix-command", "flakes"],
        )
        .bind("nix.optimise.automatic", true)
        .bind("nix.gc.automatic", true)
        .bind("nix.gc.dates", "weekly")
        .bind("nix.gc.options", "--delete-older-than 30d")
        .bind("nixpkgs.config.allowUnfree", answers.allow_unfree)
}
