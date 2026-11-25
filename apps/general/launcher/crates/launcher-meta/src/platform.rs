#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    Windows,
    Osx,
}

impl Os {
    #[must_use]
    pub const fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Osx
        } else {
            Os::Linux
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Os::Linux => "linux",
            Os::Windows => "windows",
            Os::Osx => "osx",
        }
    }

    #[must_use]
    pub const fn classpath_separator(self) -> &'static str {
        match self {
            Os::Windows => ";",
            Os::Linux | Os::Osx => ":",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Arm64,
}

impl Arch {
    #[must_use]
    pub const fn current() -> Arch {
        if cfg!(target_arch = "aarch64") {
            Arch::Arm64
        } else {
            Arch::X86_64
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Arm64 => "arm64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    #[must_use]
    pub const fn current() -> Platform {
        Platform {
            os: Os::current(),
            arch: Arch::current(),
        }
    }

    #[must_use]
    pub const fn runtime_key(self) -> &'static str {
        match (self.os, self.arch) {
            (Os::Linux, Arch::X86_64) => "linux",
            (Os::Linux, Arch::Arm64) => "linux-arm64",
            (Os::Windows, Arch::X86_64) => "windows-x64",
            (Os::Windows, Arch::Arm64) => "windows-arm64",
            (Os::Osx, Arch::X86_64) => "mac-os",
            (Os::Osx, Arch::Arm64) => "mac-os-arm64",
        }
    }
}
