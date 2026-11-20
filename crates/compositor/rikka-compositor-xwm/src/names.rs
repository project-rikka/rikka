use std::borrow::Cow;
use std::rc::Rc;

use rustc_hash::FxHashMap;
use x11rb_protocol::SequenceNumber;
use x11rb_protocol::protocol::xproto::{
    Atom, AtomEnum, GetAtomNameReply, GetAtomNameRequest, InternAtomReply, InternAtomRequest,
};

use crate::atoms::Atoms;
use crate::fault::Outcome;
use crate::pending::{ExpectedReply, Pending};
use crate::wire::{NONE, Reply, Wire, parse};

const REMEMBERED: usize = 4096;
const SEEDED: [&str; 1] = ["TEXT"];

pub(crate) fn interned_up_front() -> impl Iterator<Item = &'static str> {
    Atoms::NAMES.iter().copied().chain(SEEDED)
}

#[derive(Debug)]
pub(crate) struct AtomNames {
    names: FxHashMap<Atom, Rc<str>>,
    atoms: FxHashMap<Rc<str>, Atom>,
    naming: FxHashMap<Atom, SequenceNumber>,
    interning: FxHashMap<Rc<str>, SequenceNumber>,
    known: Vec<(Atom, Rc<str>)>,
}

impl AtomNames {
    pub(crate) fn new(interned: &[Atom]) -> Self {
        let mut known: Vec<(Atom, Rc<str>)> = interned_up_front()
            .zip(interned)
            .map(|(name, atom)| (*atom, Rc::from(name)))
            .collect();
        known.push((AtomEnum::STRING.into(), Rc::from("STRING")));
        let mut names = Self {
            names: FxHashMap::default(),
            atoms: FxHashMap::default(),
            naming: FxHashMap::default(),
            interning: FxHashMap::default(),
            known,
        };
        names.reset();
        names
    }

    pub(crate) fn name(&self, atom: Atom) -> Option<&Rc<str>> {
        self.names.get(&atom)
    }

    pub(crate) fn atom(&self, name: &str) -> Option<Atom> {
        self.atoms.get(name).copied()
    }

    pub(crate) fn learn_names(
        &mut self,
        wire: &mut Wire,
        pending: &mut Pending,
        atoms: &[Atom],
    ) -> Outcome<Option<SequenceNumber>> {
        let mut last = None;
        for atom in atoms.iter().copied() {
            if atom == NONE || self.names.contains_key(&atom) {
                continue;
            }
            let sequence = match self.naming.get(&atom) {
                Some(sequence) => *sequence,
                None => {
                    let sequence = wire.ask(GetAtomNameRequest { atom })?;
                    pending.expect(sequence, ExpectedReply::AtomName { atom });
                    self.naming.insert(atom, sequence);
                    sequence
                }
            };
            last = last.max(Some(sequence));
        }
        Ok(last)
    }

    pub(crate) fn learn_atoms<'a>(
        &mut self,
        wire: &mut Wire,
        pending: &mut Pending,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Outcome<Option<SequenceNumber>> {
        let mut last = None;
        for name in names {
            if self.atoms.contains_key(name) {
                continue;
            }
            let sequence = match self.interning.get(name) {
                Some(sequence) => *sequence,
                None => {
                    let sequence = wire.ask(InternAtomRequest {
                        only_if_exists: false,
                        name: Cow::Borrowed(name.as_bytes()),
                    })?;
                    let name: Rc<str> = Rc::from(name);
                    pending.expect(
                        sequence,
                        ExpectedReply::Atom {
                            name: Rc::clone(&name),
                        },
                    );
                    self.interning.insert(name, sequence);
                    sequence
                }
            };
            last = last.max(Some(sequence));
        }
        Ok(last)
    }

    pub(crate) fn named(&mut self, atom: Atom, reply: Reply) -> Outcome {
        self.naming.remove(&atom);
        let Ok(packet) = reply else {
            return Ok(());
        };
        let reply: GetAtomNameReply = parse(&packet)?;
        let name: Rc<str> = Rc::from(String::from_utf8_lossy(&reply.name));
        self.remember(atom, name);
        Ok(())
    }

    pub(crate) fn interned(&mut self, name: Rc<str>, reply: Reply) -> Outcome {
        self.interning.remove(&name);
        let Ok(packet) = reply else {
            return Ok(());
        };
        let reply: InternAtomReply = parse(&packet)?;
        self.remember(reply.atom, name);
        Ok(())
    }

    fn remember(&mut self, atom: Atom, name: Rc<str>) {
        if self.names.len() >= REMEMBERED {
            self.reset();
        }
        self.atoms.insert(Rc::clone(&name), atom);
        self.names.insert(atom, name);
    }

    fn reset(&mut self) {
        self.names.clear();
        self.atoms.clear();
        for (atom, name) in &self.known {
            self.atoms.insert(Rc::clone(name), *atom);
            self.names.insert(*atom, Rc::clone(name));
        }
    }
}
