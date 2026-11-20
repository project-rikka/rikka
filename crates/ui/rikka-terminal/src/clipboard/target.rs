use rikka_sync::{Mutex, MutexGuard};

use rikka_ui::Selection;

use crate::clipboard::selection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Session(Selection),
    Local(usize),
}

const SECONDARY: usize = 8;
const LOCAL_SELECTIONS: usize = SECONDARY + 1;

static LOCAL_SELECTION_DATA: Mutex<[Vec<u8>; LOCAL_SELECTIONS]> =
    Mutex::new([const { Vec::new() }; LOCAL_SELECTIONS]);

impl Target {
    pub(crate) fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            b'c' => Self::Session(Selection::Clipboard),
            b'p' | b's' => Self::Session(Selection::Primary),
            b'q' => Self::Local(SECONDARY),
            b'0'..=b'7' => Self::Local(usize::from(byte - b'0')),
            _ => return None,
        })
    }

    pub(crate) fn contents(self) -> Vec<u8> {
        match self {
            Self::Session(selection) => rikka_ui::clipboard::text_of(selection)
                .unwrap_or_default()
                .into_bytes(),
            Self::Local(index) => local_selections()[index].clone(),
        }
    }

    pub(crate) fn store(self, data: &[u8]) {
        match self {
            Self::Session(target) => match str::from_utf8(data) {
                Ok(text) => selection::write(target, text),
                Err(error) => {
                    tracing::warn!("ignoring non-UTF-8 clipboard data: {error}");
                }
            },
            Self::Local(index) => local_selections()[index] = data.to_vec(),
        }
    }
}

fn local_selections() -> MutexGuard<'static, [Vec<u8>; LOCAL_SELECTIONS]> {
    LOCAL_SELECTION_DATA.lock()
}
