use installer_core::{Answers, ByteSize, Disk, DiskKind, Secret};
use installer_sys::{Command, DemoHost, Event, Host, Mode, Output, Result, install};

pub(crate) fn answers() -> Answers {
    Answers {
        hostname: "workstation".to_owned(),
        username: "eldodebug".to_owned(),
        password: Secret::from("correct horse battery"),
        password_confirm: Secret::from("correct horse battery"),
        disk: Some(Disk {
            name: "nvme0n1".to_owned(),
            path: "/dev/nvme0n1".to_owned(),
            size: ByteSize::from_gib(476),
            model: "Samsung SSD 980 PRO".to_owned(),
            kind: DiskKind::Nvme,
            removable: false,
            read_only: false,
            in_use: false,
        }),
        ..Answers::default()
    }
}

pub(crate) fn encrypted() -> Answers {
    Answers {
        encrypt: true,
        passphrase: Secret::from("a long passphrase"),
        passphrase_confirm: Secret::from("a long passphrase"),
        ..answers()
    }
}

pub(crate) fn run(host: &dyn Host, answers: &Answers) -> (Vec<String>, Vec<Event>) {
    let mut events = Vec::new();
    install(host, answers, &mut |event| events.push(event));
    (host.journal(), events)
}

pub(crate) fn finished(events: &[Event]) -> bool {
    events.iter().any(|event| matches!(event, Event::Finished))
}

pub(crate) fn position(journal: &[String], entry: &str) -> usize {
    journal
        .iter()
        .position(|line| line == entry)
        .unwrap_or_else(|| panic!("{entry} is run\n{journal:#?}"))
}

#[derive(Debug)]
pub(crate) struct FailingHost {
    inner: DemoHost,
    program: &'static str,
}

impl FailingHost {
    pub(crate) fn new(program: &'static str) -> FailingHost {
        FailingHost {
            inner: DemoHost::default(),
            program,
        }
    }
}

impl Host for FailingHost {
    fn execute(&self, command: &Command, on_line: &mut dyn FnMut(&str)) -> Result<Output> {
        let output = self.inner.execute(command, on_line)?;
        if command.program == self.program {
            return Ok(Output {
                status: 1,
                stderr: format!("{}: rigged to fail", self.program),
                ..output
            });
        }
        Ok(output)
    }

    fn capture(&self, command: &Command) -> Result<String> {
        self.inner.capture(command)
    }

    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }

    fn read(&self, path: &str) -> Result<String> {
        self.inner.read(path)
    }

    fn write(&self, path: &str, contents: &str, mode: u32) -> Result<()> {
        self.inner.write(path, contents, mode)
    }

    fn mkdir(&self, path: &str, mode: u32) -> Result<()> {
        self.inner.mkdir(path, mode)
    }

    fn which(&self, tool: &str) -> bool {
        self.inner.which(tool)
    }

    fn euid(&self) -> u32 {
        self.inner.euid()
    }

    fn mode(&self) -> Mode {
        self.inner.mode()
    }

    fn journal(&self) -> Vec<String> {
        self.inner.journal()
    }
}
