use std::collections::HashMap;

use crate::shapers::syllabic::pattern::Pattern;

const ALPHABET: usize = 64;
const DEAD: u16 = u16::MAX;
const NO_RULE: u8 = u8::MAX;
const START: u16 = 0;

#[derive(Default)]
struct Nfa {
    epsilons: Vec<Vec<u32>>,
    symbols: Vec<Vec<(u64, u32)>>,
    accepts: Vec<u8>,
}

impl Nfa {
    fn state(&mut self) -> u32 {
        self.epsilons.push(Vec::new());
        self.symbols.push(Vec::new());
        self.accepts.push(NO_RULE);
        (self.epsilons.len() - 1) as u32
    }

    fn epsilon(&mut self, from: u32, to: u32) {
        self.epsilons[from as usize].push(to);
    }

    fn fragment(&mut self, pattern: &Pattern) -> (u32, u32) {
        let start = self.state();
        let end = self.state();
        match pattern {
            Pattern::Symbols(set) => self.symbols[start as usize].push((*set, end)),
            Pattern::Sequence(parts) => {
                let mut tail = start;
                for part in parts {
                    let (first, last) = self.fragment(part);
                    self.epsilon(tail, first);
                    tail = last;
                }
                self.epsilon(tail, end);
            }
            Pattern::Choice(parts) => {
                for part in parts {
                    let (first, last) = self.fragment(part);
                    self.epsilon(start, first);
                    self.epsilon(last, end);
                }
            }
            Pattern::Repeat(part) => {
                let (first, last) = self.fragment(part);
                self.epsilon(start, end);
                self.epsilon(start, first);
                self.epsilon(last, first);
                self.epsilon(last, end);
            }
            Pattern::Optional(part) => {
                let (first, last) = self.fragment(part);
                self.epsilon(start, end);
                self.epsilon(start, first);
                self.epsilon(last, end);
            }
        }
        (start, end)
    }

    fn is_significant(&self, state: usize) -> bool {
        !self.symbols[state].is_empty() || self.accepts[state] != NO_RULE
    }

    fn closures(&self) -> Vec<Vec<u32>> {
        let count = self.accepts.len();
        let mut last_origin = vec![usize::MAX; count];
        let mut pending = Vec::new();
        (0..count)
            .map(|origin| {
                let mut closure = Vec::new();
                pending.push(origin as u32);
                last_origin[origin] = origin;
                while let Some(state) = pending.pop() {
                    if self.is_significant(state as usize) {
                        closure.push(state);
                    }
                    for &next in &self.epsilons[state as usize] {
                        if last_origin[next as usize] != origin {
                            last_origin[next as usize] = origin;
                            pending.push(next);
                        }
                    }
                }
                closure.sort_unstable();
                closure
            })
            .collect()
    }

    fn symbol_classes(&self) -> Vec<u8> {
        let mut sets: Vec<u64> = self.symbols.iter().flatten().map(|(set, _)| *set).collect();
        sets.sort_unstable();
        sets.dedup();
        let mut signatures: Vec<Vec<bool>> = Vec::new();
        (0..ALPHABET)
            .map(|symbol| {
                let signature: Vec<bool> =
                    sets.iter().map(|set| set & (1 << symbol) != 0).collect();
                let class = signatures
                    .iter()
                    .position(|known| *known == signature)
                    .unwrap_or_else(|| {
                        signatures.push(signature);
                        signatures.len() - 1
                    });
                class as u8
            })
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct SyllableMachine {
    transitions: Box<[u16]>,
    kinds: Box<[u8]>,
    fallback: u8,
}

impl SyllableMachine {
    pub(crate) fn new(rules: &[(Pattern, u8)]) -> Self {
        let mut nfa = Nfa::default();
        let start = nfa.state();
        for (index, (rule, _)) in rules.iter().enumerate() {
            let (first, last) = nfa.fragment(rule);
            nfa.epsilon(start, first);
            nfa.accepts[last as usize] = index as u8;
        }
        let closures = nfa.closures();
        let classes = nfa.symbol_classes();
        let class_count = usize::from(classes.iter().copied().max().unwrap_or(0)) + 1;
        let representatives: Vec<usize> = (0..class_count)
            .map(|class| {
                classes
                    .iter()
                    .position(|&candidate| usize::from(candidate) == class)
                    .unwrap_or(0)
            })
            .collect();

        let initial = closures[start as usize].clone();
        let mut index_of = HashMap::from([(initial.clone(), START)]);
        let mut sets = vec![initial];
        let mut transitions = Vec::new();
        let mut kinds = Vec::new();
        let mut marks = vec![usize::MAX; nfa.accepts.len()];
        let mut stamp = 0;
        let mut current = 0;
        while current < sets.len() {
            let members = sets[current].clone();
            kinds.push(
                members
                    .iter()
                    .map(|&state| nfa.accepts[state as usize])
                    .min()
                    .map_or(NO_RULE, |rule| {
                        rules
                            .get(usize::from(rule))
                            .map_or(NO_RULE, |(_, kind)| *kind)
                    }),
            );
            let mut targets = vec![DEAD; class_count];
            for (class, &representative) in representatives.iter().enumerate() {
                let bit = 1u64 << representative;
                stamp += 1;
                let mut next = Vec::new();
                for &state in &members {
                    for &(symbols, target) in &nfa.symbols[state as usize] {
                        if symbols & bit == 0 {
                            continue;
                        }
                        for &reached in &closures[target as usize] {
                            if marks[reached as usize] != stamp {
                                marks[reached as usize] = stamp;
                                next.push(reached);
                            }
                        }
                    }
                }
                if next.is_empty() {
                    continue;
                }
                next.sort_unstable();
                targets[class] = match index_of.get(&next) {
                    Some(&target) => target,
                    None => {
                        let target = sets.len() as u16;
                        index_of.insert(next.clone(), target);
                        sets.push(next);
                        target
                    }
                };
            }
            transitions.extend(classes.iter().map(|&class| targets[usize::from(class)]));
            current += 1;
        }
        Self {
            transitions: transitions.into(),
            kinds: kinds.into(),
            fallback: rules.last().map_or(0, |(_, kind)| *kind),
        }
    }

    #[inline]
    pub(crate) fn longest_match(
        &self,
        start: usize,
        end: usize,
        mut next: impl FnMut(usize) -> usize,
        mut category: impl FnMut(usize) -> u8,
    ) -> (usize, u8) {
        let mut state = START;
        let mut position = start;
        let mut found = None;
        while position < end {
            let symbol = usize::from(category(position)) & (ALPHABET - 1);
            state = self.transitions[usize::from(state) * ALPHABET + symbol];
            if state == DEAD {
                break;
            }
            position = next(position + 1);
            let kind = self.kinds[usize::from(state)];
            if kind != NO_RULE {
                found = Some((position, kind));
            }
        }
        found.unwrap_or_else(|| (next(start + 1), self.fallback))
    }
}
