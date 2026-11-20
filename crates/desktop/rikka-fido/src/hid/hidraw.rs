use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::BorrowedFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

use super::descriptor::{self, Reports};
use super::device::Device;
use super::hidraw_monitor::HidrawMonitor;
use super::hotplug::Hotplug;
use super::link::{HidLink, Transport};

const CLASS: &str = "/sys/class/hidraw";
const NODES: &str = "/dev";
const PREFIX: &str = "hidraw";
const WRITE_WAIT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub struct Hidraw {
    class: PathBuf,
    nodes: PathBuf,
}

impl Hidraw {
    #[must_use]
    pub fn new() -> Hidraw {
        Hidraw {
            class: PathBuf::from(CLASS),
            nodes: PathBuf::from(NODES),
        }
    }

    fn reports(&self, node: &str) -> io::Result<Reports> {
        let descriptor = fs::read(self.class.join(node).join("device/report_descriptor"))?;
        descriptor::fido_reports(&descriptor).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "device is not a FIDO authenticator",
            )
        })
    }

    fn describe(&self, node: &str) -> Device {
        let mut device = Device::new(self.nodes.join(node).to_string_lossy());
        let Ok(uevent) = fs::read_to_string(self.class.join(node).join("device/uevent")) else {
            return device;
        };

        for line in uevent.lines() {
            if let Some(name) = line.strip_prefix("HID_NAME=") {
                device = device.with_name(name);
            } else if let Some(id) = line.strip_prefix("HID_ID=") {
                let mut parts = id.split(':').skip(1);
                let mut hex = || {
                    parts
                        .next()
                        .and_then(|part| u32::from_str_radix(part, 16).ok())
                        .and_then(|part| u16::try_from(part).ok())
                };
                if let (Some(vendor), Some(product)) = (hex(), hex()) {
                    device = device.with_ids(vendor, product);
                }
            }
        }

        device
    }
}

impl Default for Hidraw {
    fn default() -> Hidraw {
        Hidraw::new()
    }
}

impl Transport for Hidraw {
    fn devices(&self) -> io::Result<Vec<Device>> {
        let entries = match fs::read_dir(&self.class) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };

        let mut devices: Vec<Device> = entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|node| node.starts_with(PREFIX))
            .filter(|node| self.reports(node).is_ok())
            .map(|node| self.describe(&node))
            .collect();

        devices.sort_by(|left, right| left.path().cmp(right.path()));
        Ok(devices)
    }

    fn hotplug(&self) -> io::Result<Box<dyn Hotplug>> {
        Ok(Box::new(HidrawMonitor::new()?))
    }

    fn open(&self, device: &Device) -> io::Result<Box<dyn HidLink>> {
        let path = Path::new(device.path());
        let node = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| name.starts_with(PREFIX) && path.parent() == Some(self.nodes.as_path()))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a hidraw node"))?;

        let reports = self.reports(node)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)?;

        Ok(Box::new(HidrawLink { file, reports }))
    }
}

#[derive(Debug)]
struct HidrawLink {
    file: File,
    reports: Reports,
}

impl HidrawLink {
    fn ready(
        &self,
        flags: PollFlags,
        wait: Duration,
        interrupt: Option<BorrowedFd<'_>>,
    ) -> io::Result<bool> {
        let timeout = Timespec::try_from(wait).map_err(|_| io::ErrorKind::InvalidInput)?;
        let mut fds: Vec<PollFd<'_>> = std::iter::once(PollFd::new(&self.file, flags))
            .chain(interrupt.as_ref().map(|fd| PollFd::new(fd, PollFlags::IN)))
            .collect();

        match poll(&mut fds, Some(&timeout)) {
            Ok(0) | Err(Errno::INTR) => return Ok(false),
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }

        let revents = fds[0].revents();
        if revents.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
            return Err(io::Error::from_raw_os_error(libc::ENODEV));
        }

        Ok(revents.intersects(flags))
    }
}

impl HidLink for HidrawLink {
    fn packet_len(&self) -> usize {
        self.reports.input.min(self.reports.output)
    }

    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        let mut report = vec![0u8; self.reports.output + 1];
        let length = packet.len().min(self.reports.output);
        report[1..=length].copy_from_slice(&packet[..length]);

        loop {
            match self.file.write(&report) {
                Ok(written) if written == report.len() => return Ok(()),
                Ok(_) => return Err(io::ErrorKind::WriteZero.into()),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if !self.ready(PollFlags::OUT, WRITE_WAIT, None)? {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn receive(
        &mut self,
        wait: Duration,
        interrupt: Option<BorrowedFd<'_>>,
    ) -> io::Result<Option<Vec<u8>>> {
        if !self.ready(PollFlags::IN, wait, interrupt)? {
            return Ok(None);
        }

        let mut report = vec![0u8; self.reports.input];
        match self.file.read(&mut report) {
            Ok(0) => Err(io::Error::from_raw_os_error(libc::ENODEV)),
            Ok(read) => {
                report.truncate(read);
                Ok(Some(report))
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}
