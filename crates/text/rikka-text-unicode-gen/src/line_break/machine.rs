use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use super::rules::{Context, Decision, Lookahead};
use super::unit::Unit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Action {
    Decided(Option<Decision>),
    Ahead(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Ahead {
    Decided(Option<Decision>),
    Beyond(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Column {
    pub(crate) complex: bool,
    pub(crate) attaches: bool,
    pub(crate) absorbs: bool,
}

#[derive(Debug)]
pub(crate) struct Machine {
    pub(crate) columns: Vec<Column>,
    pub(crate) kind_columns: HashMap<Unit, u8>,
    pub(crate) start: usize,
    pub(crate) states: usize,
    pub(crate) transitions: Vec<(Action, usize)>,
    pub(crate) after_tables: Vec<Vec<Ahead>>,
    pub(crate) beyond_tables: Vec<Vec<Option<Decision>>>,
}

#[derive(Debug)]
struct Interner<T> {
    values: Vec<T>,
    positions: HashMap<T, usize>,
}

impl<T: Clone + Eq + Hash> Interner<T> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            positions: HashMap::new(),
        }
    }

    fn intern(&mut self, value: T) -> usize {
        if let Some(&position) = self.positions.get(&value) {
            return position;
        }
        self.values.push(value.clone());
        self.positions.insert(value, self.values.len() - 1);
        self.values.len() - 1
    }
}

struct Explorer<'a> {
    kinds: &'a [Unit],
    upcoming: Vec<Option<Unit>>,
    after_tables: Interner<Vec<Ahead>>,
    beyond_tables: Interner<Vec<Option<Decision>>>,
}

impl Explorer<'_> {
    fn decide(&mut self, context: &Context, next: &Unit) -> Action {
        let probe = Lookahead::new(&[]);
        let decision = context.opportunity(next, &probe);
        if probe.deepest() == 0 {
            return Action::Decided(decision);
        }
        let upcoming = self.upcoming.clone();
        let table = upcoming
            .iter()
            .map(|&after| {
                let probe = Lookahead::new(std::slice::from_ref(&after));
                let decision = context.opportunity(next, &probe);
                if probe.deepest() <= 1 || after.is_none() {
                    return Ahead::Decided(decision);
                }
                let beyond = upcoming
                    .iter()
                    .map(|&beyond| context.opportunity(next, &Lookahead::new(&[after, beyond])))
                    .collect();
                Ahead::Beyond(self.beyond_tables.intern(beyond))
            })
            .collect();
        Action::Ahead(self.after_tables.intern(table))
    }

    fn step(&mut self, context: &Context, kind: Unit) -> (Action, Context) {
        if let Some(absorbed) = context.absorb(kind) {
            return (Action::Decided(None), absorbed);
        }
        let next = kind.as_base();
        (self.decide(context, &next), context.advance(next))
    }

    fn explore(&mut self) -> (Vec<(Action, usize)>, usize) {
        let mut states = Interner::new();
        let start = states.intern(Context::default());
        let mut transitions = Vec::new();
        let mut index = 0;
        while index < states.values.len() {
            let context = states.values[index];
            for &kind in self.kinds {
                let (action, target) = self.step(&context, kind);
                transitions.push((action, states.intern(target)));
            }
            index += 1;
        }
        (transitions, start)
    }
}

fn refine(transitions: &[(Action, usize)], inputs: usize) -> Vec<usize> {
    let states = transitions.len() / inputs;
    let mut blocks = vec![0usize; states];
    let mut count = 1;
    loop {
        let mut signatures = Interner::new();
        let next: Vec<usize> = (0..states)
            .map(|state| {
                let row = &transitions[state * inputs..(state + 1) * inputs];
                let signature: Vec<(Action, usize)> = row
                    .iter()
                    .map(|&(action, target)| (action, blocks[target]))
                    .collect();
                signatures.intern((blocks[state], signature))
            })
            .collect();
        let refined = signatures.values.len();
        blocks = next;
        if refined == count {
            return blocks;
        }
        count = refined;
    }
}

fn renumber(
    transitions: &[(Action, usize)],
    inputs: usize,
    blocks: &[usize],
    start: usize,
) -> (Vec<(Action, usize)>, usize) {
    let mut order: HashMap<usize, usize> = HashMap::new();
    let mut representatives = Vec::new();
    let mut queue = VecDeque::from([start]);
    order.insert(blocks[start], 0);
    representatives.push(start);
    while let Some(state) = queue.pop_front() {
        for &(_, target) in &transitions[state * inputs..(state + 1) * inputs] {
            if let Entry::Vacant(slot) = order.entry(blocks[target]) {
                slot.insert(representatives.len());
                representatives.push(target);
                queue.push_back(target);
            }
        }
    }
    let minimized = representatives
        .iter()
        .flat_map(|&state| {
            transitions[state * inputs..(state + 1) * inputs]
                .iter()
                .map(|&(action, target)| (action, order[&blocks[target]]))
        })
        .collect();
    (minimized, representatives.len())
}

pub(crate) fn build(kinds: &[Unit]) -> Machine {
    let mut upcoming: Vec<Option<Unit>> = kinds.iter().map(|&kind| Some(kind.as_base())).collect();
    upcoming.push(None);
    let mut explorer = Explorer {
        kinds,
        upcoming,
        after_tables: Interner::new(),
        beyond_tables: Interner::new(),
    };
    let (raw, raw_start) = explorer.explore();
    let inputs = kinds.len();
    let blocks = refine(&raw, inputs);
    let (minimized, states) = renumber(&raw, inputs, &blocks, raw_start);
    let after_tables = explorer.after_tables.values;
    let beyond_tables = explorer.beyond_tables.values;

    let mut signatures = Interner::new();
    let mut columns = Vec::new();
    let mut kind_columns = HashMap::new();
    let mut representatives = Vec::new();
    for (index, &kind) in kinds.iter().enumerate() {
        let column = Column {
            complex: kind.complex,
            attaches: kind.attaches(),
            absorbs: kind.as_base().absorbs(),
        };
        let signature = (
            column.complex,
            column.attaches,
            column.absorbs,
            (0..states)
                .map(|state| minimized[state * inputs + index])
                .collect::<Vec<_>>(),
            after_tables
                .iter()
                .map(|table| table[index])
                .collect::<Vec<_>>(),
            beyond_tables
                .iter()
                .map(|table| table[index])
                .collect::<Vec<_>>(),
        );
        let id = signatures.intern(signature);
        if id == columns.len() {
            columns.push(column);
            representatives.push(index);
        }
        kind_columns.insert(kind, id as u8);
    }
    let minimized = &minimized;
    let representatives_ref = &representatives;
    let transitions = (0..states)
        .flat_map(|state| {
            representatives_ref
                .iter()
                .map(move |&index| minimized[state * inputs + index])
        })
        .collect();
    Machine {
        columns,
        kind_columns,
        start: 0,
        states,
        transitions,
        after_tables: after_tables
            .iter()
            .map(|table| with_end(&representatives, table))
            .collect(),
        beyond_tables: beyond_tables
            .iter()
            .map(|table| with_end(&representatives, table))
            .collect(),
    }
}

fn with_end<T: Copy>(representatives: &[usize], table: &[T]) -> Vec<T> {
    representatives
        .iter()
        .map(|&index| table[index])
        .chain(table.last().copied())
        .collect()
}
