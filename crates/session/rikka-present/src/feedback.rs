use std::os::fd::{AsRawFd, OwnedFd};

use rikka_gpu::DRM_FORMAT_MOD_LINEAR;
use rikka_gpu::DrmFormat;

const TABLE_ENTRY: usize = 16;

#[derive(Clone, Debug, Default)]
pub(crate) struct Feedback {
    pub(crate) main: Option<u64>,
    pub(crate) tranches: Vec<Tranche>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Tranche {
    pub(crate) device: Option<u64>,
    pub(crate) scanout: bool,
    pub(crate) formats: Vec<DrmFormat>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) fourcc: u32,
    pub(crate) modifiers: Vec<u64>,
    pub(crate) scanout: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Gathering {
    table: Vec<DrmFormat>,
    main: Option<u64>,
    tranches: Vec<Tranche>,
    tranche: Tranche,
}

impl Gathering {
    pub(crate) fn table(&mut self, fd: &OwnedFd, size: u32) {
        self.table = read_table(fd, size as usize).unwrap_or_else(|error| {
            tracing::warn!("failed to map dmabuf format table: {error}");
            Vec::new()
        });
    }

    pub(crate) fn main_device(&mut self, device: &[u8]) {
        self.main = device_number(device);
    }

    pub(crate) fn tranche_device(&mut self, device: &[u8]) {
        self.tranche.device = device_number(device);
    }

    pub(crate) fn tranche_formats(&mut self, indices: &[u8]) {
        let table = &self.table;
        self.tranche.formats.extend(
            indices
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&index| usize::from(u16::from_ne_bytes(index)))
                .filter_map(|index| table.get(index).copied()),
        );
    }

    pub(crate) fn tranche_scanout(&mut self, scanout: bool) {
        self.tranche.scanout = scanout;
    }

    pub(crate) fn tranche_done(&mut self) {
        self.tranches.push(std::mem::take(&mut self.tranche));
    }

    pub(crate) fn done(&mut self) -> Feedback {
        self.tranche = Tranche::default();
        Feedback {
            main: self.main,
            tranches: std::mem::take(&mut self.tranches),
        }
    }
}

impl Feedback {
    pub(crate) fn choices(
        &self,
        fourccs: &[u32],
        exportable: &[DrmFormat],
        local: bool,
    ) -> Vec<Choice> {
        self.tranches
            .iter()
            .filter_map(|tranche| {
                fourccs.iter().find_map(|&fourcc| {
                    let modifiers: Vec<u64> = tranche
                        .formats
                        .iter()
                        .filter(|format| format.fourcc == fourcc)
                        .filter(|format| local || format.modifier == DRM_FORMAT_MOD_LINEAR)
                        .filter(|format| exportable.contains(format))
                        .map(|format| format.modifier)
                        .collect();
                    (!modifiers.is_empty()).then_some(Choice {
                        fourcc,
                        modifiers,
                        scanout: tranche.scanout,
                    })
                })
            })
            .collect()
    }
}

fn device_number(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_ne_bytes(bytes.try_into().ok()?))
}

fn read_table(fd: &OwnedFd, size: usize) -> std::io::Result<Vec<DrmFormat>> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let mapped = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            fd.as_raw_fd(),
            0,
        )
    };
    if mapped == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }
    let bytes = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), size) };
    let table = bytes
        .as_chunks::<TABLE_ENTRY>()
        .0
        .iter()
        .map(|entry| {
            let (fourcc, rest) = entry
                .split_first_chunk::<4>()
                .expect("format table entry should hold a fourcc");
            let (_, modifier) = rest
                .split_last_chunk::<8>()
                .expect("format table entry should hold a modifier");
            DrmFormat {
                fourcc: u32::from_ne_bytes(*fourcc),
                modifier: u64::from_ne_bytes(*modifier),
            }
        })
        .collect();
    unsafe { libc::munmap(mapped, size) };
    Ok(table)
}
