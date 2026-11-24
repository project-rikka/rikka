use std::path::Path;

use installer_core::{
    Answers, ByteSize, Filesystem, Location, PartitionPlan, PartitionRole, PartitionSpec,
    SECRETS_DIR, SECRETS_MODE, Swap, TARGET_ROOT, generate,
};

use crate::{command::Command, error::Result, host::Host, stage::Stage};

const UDEV_TIMEOUT: &str = "--timeout=30";

#[derive(Clone, Debug)]
pub enum Event {
    Planned(Vec<Stage>),
    Entered(Stage),
    Log(String),
    Finished,
    Failed(String),
}

pub fn install(host: &dyn Host, answers: &Answers, report: &mut dyn FnMut(Event)) {
    let plan = match PartitionPlan::build(answers) {
        Ok(plan) => plan,
        Err(error) => {
            report(Event::Failed(error.to_string()));
            return;
        }
    };

    let stages = Stage::plan(answers);
    report(Event::Planned(stages.clone()));

    let mut run = Run {
        host,
        answers,
        plan,
        acquired: Acquired::default(),
        report,
    };

    match run.all(&stages) {
        Ok(()) => run.report(Event::Finished),
        Err(error) => {
            run.roll_back();
            run.report(Event::Failed(error.to_string()));
        }
    }
}

struct Run<'a> {
    host: &'a dyn Host,
    answers: &'a Answers,
    plan: PartitionPlan,
    acquired: Acquired,
    report: &'a mut dyn FnMut(Event),
}

#[derive(Default)]
struct Acquired {
    swap: bool,
    luks: bool,
    mounts: bool,
}

impl Run<'_> {
    fn all(&mut self, stages: &[Stage]) -> Result<()> {
        for stage in stages {
            self.report(Event::Entered(*stage));
            match stage {
                Stage::Partition => self.partition()?,
                Stage::Encrypt => self.encrypt()?,
                Stage::Format => self.format()?,
                Stage::Mount => self.mount()?,
                Stage::Scan => self.scan()?,
                Stage::Configure => self.configure()?,
                Stage::Install => self.nixos_install()?,
                Stage::Finish => self.finish()?,
            }
        }
        Ok(())
    }

    fn report(&mut self, event: Event) {
        (self.report)(event);
    }

    fn log(&mut self, line: impl Into<String>) {
        self.report(Event::Log(line.into()));
    }

    fn run(&mut self, command: Command) -> Result<()> {
        self.log(format!("$ {command}"));
        let report = &mut *self.report;
        self.host.run_logged(&command, &mut |line| {
            report(Event::Log(line.to_owned()));
        })?;
        Ok(())
    }

    fn partition(&mut self) -> Result<()> {
        let disk = self.plan.disk.path.clone();

        self.run(Command::new("wipefs").args(["--all", "--force", &disk]))?;
        self.run(Command::new("sgdisk").args(["--zap-all", &disk]))?;

        for spec in self.plan.partitions.clone() {
            let range = match spec.size {
                Some(size) => format!("{}:0:+{}M", spec.number, size.bytes() / ByteSize::MIB),
                None => format!("{}:0:0", spec.number),
            };
            self.run(Command::new("sgdisk").args([
                format!("--new={range}"),
                format!("--typecode={}:{}", spec.number, spec.type_code),
                format!("--change-name={}:{}", spec.number, spec.label),
                disk.clone(),
            ]))?;
        }

        self.wait_for_nodes()?;

        for node in self.nodes() {
            self.run(Command::new("wipefs").args(["--all", "--force", &node]))?;
        }

        self.wait_for_nodes()
    }

    fn settle(&mut self) -> Result<()> {
        self.run(Command::new("udevadm").args(["settle", UDEV_TIMEOUT]))
    }

    fn nodes(&self) -> Vec<String> {
        self.plan
            .partitions
            .iter()
            .map(|spec| spec.node.clone())
            .collect()
    }

    fn wait_for_nodes(&mut self) -> Result<()> {
        let nodes = self.nodes();
        self.run(
            Command::new("udevadm")
                .args(["wait", "--settle", UDEV_TIMEOUT])
                .args(nodes),
        )
    }

    fn roll_back(&mut self) {
        if !self.acquired.swap && !self.acquired.luks && !self.acquired.mounts {
            return;
        }
        self.log("Rolling back".to_owned());

        if self.acquired.swap
            && let Some(spec) = self.plan.find(PartitionRole::Swap).cloned()
        {
            self.run_best_effort(Command::new("swapoff").arg(&spec.node));
        }
        if self.acquired.mounts {
            self.run_best_effort(Command::new("umount").args(["--recursive", TARGET_ROOT]));
        }
        if self.acquired.luks {
            let name = self.plan.luks_name.to_owned();
            self.run_best_effort(Command::new("cryptsetup").args(["close", &name]));
        }
    }

    fn run_best_effort(&mut self, command: Command) {
        self.log(format!("$ {command}"));
        let report = &mut *self.report;
        let outcome = self.host.execute(&command, &mut |line| {
            report(Event::Log(line.to_owned()));
        });
        match outcome {
            Ok(output) if output.succeeded() => {}
            Ok(output) => self.log(format!("failed: {command} (exit {})", output.status)),
            Err(error) => self.log(format!("failed: {error}")),
        }
    }

    fn encrypt(&mut self) -> Result<()> {
        let node = self.plan.root().node.clone();
        let name = self.plan.luks_name.to_owned();

        self.log("Creating the LUKS2 container (argon2id takes a few seconds)".to_owned());
        self.run(
            Command::new("cryptsetup")
                .args([
                    "luksFormat",
                    "--type",
                    "luks2",
                    "--pbkdf",
                    "argon2id",
                    "--batch-mode",
                    "--key-file",
                    "-",
                    &node,
                ])
                .stdin(self.answers.passphrase.clone()),
        )?;

        let mut open = Command::new("cryptsetup").args(["open", "--key-file", "-"]);
        if self.plan.disk.kind.trims() {
            open = open.args(["--persistent", "--allow-discards"]);
        }
        self.run(
            open.args([&node, &name])
                .stdin(self.answers.passphrase.clone()),
        )?;
        self.acquired.luks = true;
        Ok(())
    }

    fn format(&mut self) -> Result<()> {
        let esp = self.plan.esp().node.clone();
        self.run(Command::new("mkfs.fat").args(["-F", "32", "-n", "ESP", &esp]))?;

        if let Some(swap) = self.plan.find(PartitionRole::Swap).cloned() {
            self.make_swap(&swap)?;
        }

        let root = self.plan.root_device();
        match self.answers.filesystem {
            Filesystem::Ext4 => {
                self.run(Command::new("mkfs.ext4").args(["-F", "-L", "nixos", &root]))?;
            }
            Filesystem::Btrfs => {
                self.run(Command::new("mkfs.btrfs").args(["--force", "--label", "nixos", &root]))?;
            }
        }

        self.settle()?;

        if self.answers.filesystem == Filesystem::Btrfs {
            self.make_subvolumes(&root)?;
        }
        Ok(())
    }

    fn make_swap(&mut self, spec: &PartitionSpec) -> Result<()> {
        self.run(Command::new("mkswap").args(["--label", "swap", &spec.node]))?;
        self.run(Command::new("swapon").arg(&spec.node))?;
        self.acquired.swap = true;
        Ok(())
    }

    fn make_subvolumes(&mut self, root: &str) -> Result<()> {
        let options = Filesystem::Btrfs.base_options().join(",");
        self.mount_device(Filesystem::Btrfs.label(), &options, root, TARGET_ROOT)?;
        for (subvolume, _) in Filesystem::SUBVOLUMES {
            self.run(Command::new("btrfs").args([
                "subvolume",
                "create",
                &format!("{TARGET_ROOT}/{subvolume}"),
            ]))?;
        }
        self.run(Command::new("umount").arg(TARGET_ROOT))?;
        self.acquired.mounts = false;
        Ok(())
    }

    fn mount_device(&mut self, kind: &str, options: &str, device: &str, at: &str) -> Result<()> {
        self.run(Command::new("mount").args(["-t", kind, "-o", options, device, at]))?;
        self.acquired.mounts = true;
        Ok(())
    }

    fn mount(&mut self) -> Result<()> {
        let root = self.plan.root_device();
        let kind = self.answers.filesystem.label();
        let base = self.answers.filesystem.base_options().join(",");

        match self.answers.filesystem {
            Filesystem::Ext4 => self.mount_device(kind, &base, &root, TARGET_ROOT)?,
            Filesystem::Btrfs => {
                for (subvolume, at) in Filesystem::SUBVOLUMES {
                    let target = format!("{TARGET_ROOT}{at}");
                    if !at.is_empty() {
                        self.mkdir(&target, 0o755)?;
                    }
                    self.mount_device(kind, &format!("{base},subvol={subvolume}"), &root, &target)?;
                }
            }
        }

        let boot = format!("{TARGET_ROOT}/boot");
        self.mkdir(&boot, 0o755)?;
        let esp = self.plan.esp().node.clone();
        self.mount_device("vfat", "fmask=0077,dmask=0077", &esp, &boot)
    }

    fn scan(&mut self) -> Result<()> {
        self.log("Generating hardware configuration".to_owned());
        self.run(Command::new("nixos-generate-config").args(["--root", TARGET_ROOT]))
    }

    fn configure(&mut self) -> Result<()> {
        let files = generate(self.answers)?;

        self.mkdir(&format!("{TARGET_ROOT}{SECRETS_DIR}"), SECRETS_MODE)?;
        let location = Location::standard();
        self.mkdir(&on_target(location.flake()), 0o755)?;
        self.mkdir(&on_target(location.modules()), 0o755)?;

        for file in files {
            let path = format!("{TARGET_ROOT}{}", file.path);
            self.host.write(&path, &file.contents, file.mode)?;
            if file.secret {
                self.log(format!("wrote {path} ({:o})", file.mode));
            } else {
                self.log(format!(
                    "wrote {path} ({} lines)",
                    file.contents.lines().count()
                ));
            }
        }
        Ok(())
    }

    fn lock_flake(&mut self) -> Result<()> {
        let directory = on_target(Location::standard().flake());
        self.log("Locking flake inputs".to_owned());
        self.run(Command::nix().args(["flake", "lock", &directory]))
    }

    fn nixos_install(&mut self) -> Result<()> {
        self.lock_flake()?;

        let flake = format!(
            "{}#{}",
            on_target(Location::standard().flake()),
            self.answers.hostname
        );
        let command = Command::new("nixos-install").args([
            "--root",
            TARGET_ROOT,
            "--flake",
            &flake,
            "--no-root-passwd",
            "--no-channel-copy",
        ]);

        self.log(format!("$ {command}"));
        self.log("Building the system; this can take a long time".to_owned());

        let report = &mut *self.report;

        self.host.run_logged(&command, &mut |line| {
            report(Event::Log(line.to_owned()));
        })?;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.run(Command::new("sync"))?;
        if let Swap::Partition { .. } = self.answers.swap
            && let Some(spec) = self.plan.find(PartitionRole::Swap).cloned()
        {
            self.run(Command::new("swapoff").arg(&spec.node))?;
        }
        self.log(format!(
            "Installation complete. Reboot; Rikka starts on tty1 as {}.",
            self.answers.username
        ));
        Ok(())
    }

    fn mkdir(&mut self, path: &str, mode: u32) -> Result<()> {
        self.log(format!("mkdir -m {mode:o} {path}"));
        self.host.mkdir(path, mode)
    }
}

fn on_target(path: &Path) -> String {
    format!("{TARGET_ROOT}{}", path.display())
}
