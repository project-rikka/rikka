use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct PrivateBus {
    daemon: Child,
    address: String,
}

impl PrivateBus {
    #[must_use]
    pub fn start() -> Option<PrivateBus> {
        match PrivateBus::spawn() {
            Ok(bus) => Some(bus),
            Err(error) => {
                eprintln!("bus test skipped: dbus-daemon did not start: {error}");
                None
            }
        }
    }

    fn spawn() -> io::Result<PrivateBus> {
        let folder = std::env::temp_dir();
        let process = std::process::id();
        let log = folder.join(format!("dbus-daemon.{process}.log"));
        let config = folder.join(format!("dbus-daemon.{process}.conf"));
        fs::write(&config, session_config(&folder.display().to_string()))?;
        let mut daemon = Command::new("dbus-daemon")
            .args(["--nofork", "--print-address"])
            .arg(format!("--config-file={}", config.display()))
            .stdout(Stdio::piped())
            .stderr(File::create(&log)?)
            .spawn()?;
        let mut address = String::new();
        if let Some(stdout) = daemon.stdout.take() {
            BufReader::new(stdout).read_line(&mut address)?;
        }
        let address = address.trim();
        if address.is_empty() {
            let _ = daemon.kill();
            let _ = daemon.wait();
            let said = fs::read_to_string(&log).unwrap_or_default();
            return Err(io::Error::other(match said.trim() {
                "" => "it printed no address".to_owned(),
                said => said.to_owned(),
            }));
        }
        unsafe {
            std::env::set_var("DBUS_SESSION_BUS_ADDRESS", address);
        }
        Ok(PrivateBus {
            daemon,
            address: address.to_owned(),
        })
    }

    pub fn stand_in_for_system_bus(&self) {
        unsafe {
            std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &self.address);
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn session_config(folder: &str) -> String {
    format!(
        r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir={folder}</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#
    )
}
