use rikka_nix_edit::{Expr, Set};

use crate::{answers::Answers, error::Result};

pub(crate) fn body(answers: &Answers) -> Result<Set> {
    let set = Set::new()
        .bind("boot.loader.systemd-boot.enable", true)
        .bind("boot.loader.systemd-boot.configurationLimit", 10)
        .bind("boot.loader.efi.canTouchEfiVariables", true);
    Ok(match answers.kernel.packages() {
        Some(packages) => set.bind("boot.kernelPackages", Expr::reference(["pkgs", packages])?),
        None => set,
    })
}
