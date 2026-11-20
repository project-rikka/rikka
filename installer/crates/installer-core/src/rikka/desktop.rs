use rikka_nix_edit::{Expr, Set};
use rikka_nixos_layout::Packages;

use crate::{
    answers::Answers,
    error::Result,
    gpu::{GpuVendor, NvidiaDriver},
    rikka::module::section,
};

pub(crate) fn body(answers: &Answers) -> Result<Set> {
    let mut set = graphics(answers, Set::new())?;
    if let Some(driver) = answers.nvidia_driver() {
        set = nvidia(driver, set)?;
    }
    Ok(section(set, "Rikka")
        .bind("rikka.enable", true)
        .bind("rikka.autologin.user", &answers.username))
}

fn graphics(answers: &Answers, set: Set) -> Result<Set> {
    let mut set = section(set, "Graphics").bind("hardware.graphics.enable32Bit", true);
    if answers.has_gpu(GpuVendor::Intel) {
        set = set
            .bind(
                "hardware.graphics.extraPackages",
                Packages::new(["intel-media-driver"]),
            )
            .bind("environment.sessionVariables.LIBVA_DRIVER_NAME", "iHD");
    }
    let early = answers.early_kms();
    if !early.is_empty() {
        set = set.bind("boot.initrd.kernelModules", Expr::list(early));
    }
    Ok(set)
}

fn nvidia(driver: NvidiaDriver, set: Set) -> Result<Set> {
    Ok(section(set, "NVIDIA")
        .bind("services.xserver.videoDrivers", ["nvidia"])
        .bind("hardware.nvidia.open", driver.open())
        .bind("hardware.nvidia.modesetting.enable", true)
        .bind(
            "hardware.nvidia.package",
            Expr::reference([
                "config",
                "boot",
                "kernelPackages",
                "nvidiaPackages",
                driver.package(),
            ])?,
        )
        .bind(
            "boot.kernelModules",
            ["nvidia", "nvidia_modeset", "nvidia_drm"],
        ))
}
