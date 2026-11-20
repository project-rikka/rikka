use rikka_nix_edit::Set;
use rikka_nixos_layout::Packages;

pub const BASE_PACKAGES: &[&str] = &["curl", "git", "wget"];

pub(crate) fn body() -> Set {
    Set::new().bind("environment.systemPackages", Packages::new(BASE_PACKAGES))
}
