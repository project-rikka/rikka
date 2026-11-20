use std::fmt;
use std::rc::Rc;

pub use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::TrancheFlags;
use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use rikka_compositor_types::Format;
use rustix::fs::Dev;

use super::buffer::device_bytes;
use super::table::FormatTable;

const SAMPLING_SINCE: u32 = 6;
const MAIN_DEVICE_UNTIL: u32 = 6;
const INDICES_PER_EVENT: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmabufTranche {
    pub device: Dev,
    pub flags: TrancheFlags,
    pub formats: Vec<Format>,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedbackError {
    #[error("dmabuf feedback needs at least one tranche")]
    NoTranches,
    #[error("dmabuf feedback needs a sampling tranche")]
    NoSamplingTranche,
    #[error("every dmabuf feedback tranche needs at least one flag and one format")]
    EmptyTranche,
    #[error("failed to build dmabuf format table: {0}")]
    Table(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct DmabufFeedback(Rc<Inner>);

struct Inner {
    main_device: Dev,
    tranches: Vec<(Dev, TrancheFlags, Vec<u16>)>,
    table: FormatTable,
}

impl DmabufFeedback {
    pub fn new(main_device: Dev, tranches: Vec<DmabufTranche>) -> Result<Self, FeedbackError> {
        if tranches.is_empty() {
            return Err(FeedbackError::NoTranches);
        }
        if tranches
            .iter()
            .any(|tranche| tranche.flags.is_empty() || tranche.formats.is_empty())
        {
            return Err(FeedbackError::EmptyTranche);
        }
        if !tranches
            .iter()
            .any(|tranche| tranche.flags.contains(TrancheFlags::SAMPLING))
        {
            return Err(FeedbackError::NoSamplingTranche);
        }
        let formats: Vec<Format> = tranches
            .iter()
            .flat_map(|tranche| tranche.formats.iter().copied())
            .collect();
        let table = FormatTable::new(&formats)?;
        let tranches = tranches
            .into_iter()
            .map(|tranche| {
                let mut indices: Vec<u16> = tranche
                    .formats
                    .iter()
                    .filter_map(|format| table.index(format))
                    .collect();
                indices.sort_unstable();
                indices.dedup();
                (tranche.device, tranche.flags, indices)
            })
            .collect();
        Ok(Self(Rc::new(Inner {
            main_device,
            tranches,
            table,
        })))
    }

    pub fn main_device(&self) -> Dev {
        self.0.main_device
    }

    pub fn supports(&self, format: &Format) -> bool {
        self.0.table.contains(format)
    }

    pub(crate) fn formats(&self) -> impl Iterator<Item = &Format> {
        self.0.table.formats()
    }

    pub(crate) fn sorted(&self) -> impl Iterator<Item = Format> + '_ {
        self.0.table.sorted()
    }

    pub(crate) fn same(&self, other: &DmabufFeedback) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn send(&self, object: &ZwpLinuxDmabufFeedbackV1) {
        let inner = &self.0;
        object.format_table(inner.table.fd(), inner.table.size());
        if object.version() < MAIN_DEVICE_UNTIL {
            object.main_device(&device_bytes(inner.main_device));
        }
        for (device, flags, indices) in &inner.tranches {
            object.tranche_target_device(&device_bytes(*device));
            let flags = if object.version() < SAMPLING_SINCE {
                *flags & !TrancheFlags::SAMPLING
            } else {
                *flags
            };
            object.tranche_flags(flags);
            for chunk in indices.chunks(INDICES_PER_EVENT) {
                let bytes: Vec<u8> = chunk.iter().flat_map(|index| index.to_ne_bytes()).collect();
                object.tranche_formats(&bytes);
            }
            object.tranche_done();
        }
        object.done();
    }
}

impl fmt::Debug for DmabufFeedback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DmabufFeedback")
            .field("main_device", &self.0.main_device)
            .field("tranches", &self.0.tranches.len())
            .finish_non_exhaustive()
    }
}
