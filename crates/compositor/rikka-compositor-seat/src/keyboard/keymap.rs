use std::cell::RefCell;
use std::fmt;
use std::fs::File;
use std::hash::BuildHasher;
use std::io::Write;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_hash::{FxBuildHasher, FxHashMap};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, memfd_create};
use xkbcommon::xkb;

use crate::error::{Error, Result};
use crate::keyboard::modifiers::ModMasks;

const LIMIT: u64 = 4 << 20;

static STAMPS: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static INTERNED: RefCell<Interned> = RefCell::default();
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct KeymapNames {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
}

impl KeymapNames {
    pub fn layout(layout: impl Into<String>) -> Self {
        Self {
            layout: layout.into(),
            ..Self::default()
        }
    }

    fn describe(&self) -> String {
        let mut named = format!(
            "{}/{}/{}/{}",
            self.rules, self.model, self.layout, self.variant
        );
        if let Some(options) = &self.options {
            named.push('/');
            named.push_str(options);
        }
        named
    }
}

#[derive(Clone)]
pub struct Keymap(Rc<Compiled>);

struct Compiled {
    xkb: xkb::Keymap,
    masks: ModMasks,
    text: String,
    file: OwnedFd,
    size: u32,
    stamp: u64,
}

impl Keymap {
    pub fn from_names(names: &KeymapNames) -> Result<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            &names.rules,
            &names.model,
            &names.layout,
            &names.variant,
            names.options.clone(),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| Error::UnknownLayout {
            names: names.describe(),
        })?;
        Self::share(keymap, None)
    }

    pub fn from_text(text: &str) -> Result<Self> {
        if let Some(known) = lookup(text) {
            return Ok(known);
        }
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            text.to_owned(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(Error::MalformedKeymap)?;
        Self::share(keymap, Some(text))
    }

    pub(crate) fn from_fd(fd: OwnedFd, size: u32) -> Result<Self> {
        let file = File::from(fd);
        let length = file
            .metadata()
            .map_err(Error::UnreadableKeymap)?
            .len()
            .min(u64::from(size));
        if length > LIMIT {
            return Err(Error::KeymapTooLarge {
                size: length,
                limit: LIMIT,
            });
        }
        let mut bytes = vec![0; length as usize];
        file.read_exact_at(&mut bytes, 0)
            .map_err(Error::UnreadableKeymap)?;
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        let text = std::str::from_utf8(&bytes[..end]).map_err(|_| Error::MalformedKeymap)?;
        Self::from_text(text)
    }

    fn share(keymap: xkb::Keymap, spelled: Option<&str>) -> Result<Self> {
        let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        if let Some(known) = lookup(&text) {
            intern(&known, spelled);
            return Ok(known);
        }
        let file = sealed(&text).map_err(Error::UnsharableKeymap)?;
        let size = u32::try_from(text.len() + 1).map_err(|_| Error::KeymapTooLarge {
            size: text.len() as u64,
            limit: LIMIT,
        })?;
        let shared = Self(Rc::new(Compiled {
            masks: ModMasks::from_keymap(&keymap),
            xkb: keymap,
            text,
            file,
            size,
            stamp: STAMPS.fetch_add(1, Ordering::Relaxed),
        }));
        intern(&shared, spelled);
        Ok(shared)
    }

    pub fn text(&self) -> &str {
        &self.0.text
    }

    pub fn layouts(&self) -> Vec<String> {
        let xkb = &self.0.xkb;
        (0..xkb.num_layouts())
            .map(|layout| xkb.layout_get_name(layout).to_owned())
            .collect()
    }

    pub(crate) fn xkb(&self) -> &xkb::Keymap {
        &self.0.xkb
    }

    pub(crate) fn masks(&self) -> &ModMasks {
        &self.0.masks
    }

    pub(crate) fn file(&self) -> (BorrowedFd<'_>, u32) {
        (self.0.file.as_fd(), self.0.size)
    }

    pub(crate) fn stamp(&self) -> KeymapStamp {
        KeymapStamp(self.0.stamp)
    }
}

impl PartialEq for Keymap {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Keymap {}

impl fmt::Debug for Keymap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Keymap")
            .field("layouts", &self.layouts())
            .field("size", &self.0.size)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct KeymapStamp(u64);

impl rikka_wayland_wire::Key for KeymapStamp {
    fn into_raw(self) -> u64 {
        self.0
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Default)]
struct Interned {
    entries: FxHashMap<u64, Vec<InternedText>>,
}

struct InternedText {
    alias: Option<Box<str>>,
    keymap: Weak<Compiled>,
}

impl InternedText {
    fn matches(&self, text: &str) -> Option<Keymap> {
        let keymap = self.keymap.upgrade()?;
        let spelled = self.alias.as_deref().unwrap_or(&keymap.text);
        (spelled == text).then_some(Keymap(keymap))
    }

    fn refers_to(&self, keymap: &Keymap) -> bool {
        std::ptr::eq(self.keymap.as_ptr(), Rc::as_ptr(&keymap.0))
    }
}

fn lookup(text: &str) -> Option<Keymap> {
    INTERNED.with_borrow(|interned| {
        interned
            .entries
            .get(&FxBuildHasher.hash_one(text))?
            .iter()
            .find_map(|entry| entry.matches(text))
    })
}

fn intern(keymap: &Keymap, spelled: Option<&str>) {
    let alias = spelled.filter(|spelled| *spelled != keymap.text());
    INTERNED.with_borrow_mut(|interned| {
        let entries = &mut interned.entries;
        entries.retain(|_, bucket| {
            bucket.retain(|entry| {
                let replaced = alias.is_some() && entry.alias.is_some() && entry.refers_to(keymap);
                entry.keymap.strong_count() > 0 && !replaced
            });
            !bucket.is_empty()
        });
        let canonical = entries
            .entry(FxBuildHasher.hash_one(keymap.text()))
            .or_default();
        if !canonical
            .iter()
            .any(|entry| entry.alias.is_none() && entry.refers_to(keymap))
        {
            canonical.push(InternedText {
                alias: None,
                keymap: Rc::downgrade(&keymap.0),
            });
        }
        if let Some(alias) = alias {
            entries
                .entry(FxBuildHasher.hash_one(alias))
                .or_default()
                .push(InternedText {
                    alias: Some(alias.into()),
                    keymap: Rc::downgrade(&keymap.0),
                });
        }
    });
}

fn sealed(text: &str) -> std::io::Result<OwnedFd> {
    let fd = memfd_create(
        "rikka-keymap",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )?;
    let mut file = File::from(fd);
    file.write_all(text.as_bytes())?;
    file.write_all(&[0])?;
    let fd = OwnedFd::from(file);
    fcntl_add_seals(
        &fd,
        SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
    )?;
    Ok(fd)
}
