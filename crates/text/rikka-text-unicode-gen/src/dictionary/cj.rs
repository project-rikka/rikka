use super::dawg;
use crate::error::{Error, Result};

pub(crate) const WORD_LIST: &str = "cjdict.txt";
pub(crate) const NAME: &str = "cj";
pub(crate) const LABEL: &str = "chinese and japanese dictionary";

#[derive(Debug)]
pub(crate) struct CjDictionary {
    pub(crate) symbols: Vec<u32>,
    pub(crate) first: Vec<u32>,
    pub(crate) edges: Vec<u32>,
    pub(crate) costs: Vec<u8>,
    pub(crate) target_bits: u32,
}

impl CjDictionary {
    pub(crate) fn states(&self) -> usize {
        self.costs.len()
    }

    pub(crate) fn blob(&self) -> Vec<u8> {
        self.symbols
            .iter()
            .chain(&self.first)
            .chain(&self.edges)
            .flat_map(|word| word.to_le_bytes())
            .chain(self.costs.iter().copied())
            .collect()
    }
}

fn parse(contents: &str) -> Result<Vec<(Vec<char>, u8)>> {
    let mut entries = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let line = line.trim_start_matches('\u{FEFF}');
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let invalid = |message: String| Error::Parse {
            file: WORD_LIST.to_owned(),
            line: index + 1,
            message,
        };
        let Some((word, cost)) = line.trim_end().split_once('\t') else {
            return Err(invalid("the line has no cost".into()));
        };
        let cost = cost
            .parse::<u8>()
            .ok()
            .filter(|&cost| cost > 0)
            .ok_or_else(|| invalid(format!("{cost:?} is not a cost from 1 to 255")))?;
        if word.is_empty() {
            return Err(invalid("the line has no word".into()));
        }
        entries.push((word.chars().collect(), cost));
    }
    Ok(entries)
}

pub(crate) fn compile(contents: &str) -> Result<CjDictionary> {
    let entries = parse(contents)?;
    let mut characters: Vec<char> = entries.iter().flat_map(|(word, _)| word.clone()).collect();
    characters.sort_unstable();
    characters.dedup();
    let symbol = |character: &char| characters.partition_point(|other| other < character) as u32;
    let mut words: Vec<(Vec<u32>, Option<u8>)> = entries
        .iter()
        .map(|(word, cost)| (word.iter().map(symbol).collect(), Some(*cost)))
        .collect();
    words.sort_unstable();
    if let Some(pair) = words.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        let word: String = pair[0].0.iter().map(|&s| characters[s as usize]).collect();
        return Err(Error::Data(format!("{WORD_LIST} lists {word} twice")));
    }
    let dawg = dawg::build(words.iter().map(|(word, cost)| (word.as_slice(), *cost)));
    let order = dawg.breadth_first();
    let mut numbers = vec![0u32; dawg.states.len()];
    for (number, &state) in order.iter().enumerate() {
        numbers[state] = number as u32;
    }
    let target_bits = (order.len() - 1).max(1).ilog2() + 1;
    let label_limit = 1usize << (u32::BITS - target_bits);
    if characters.len() > label_limit {
        return Err(Error::Data(format!(
            "{WORD_LIST} needs {} states and {} characters, more than 32-bit edges hold",
            order.len(),
            characters.len()
        )));
    }
    let mut first = Vec::with_capacity(order.len() + 1);
    let mut edges = Vec::new();
    let mut costs = Vec::with_capacity(order.len());
    for &state in &order {
        first.push(edges.len() as u32);
        let state = &dawg.states[state];
        costs.push(state.accept.unwrap_or(0));
        edges.extend(
            state
                .edges
                .iter()
                .map(|&(label, target)| label << target_bits | numbers[target]),
        );
    }
    first.push(edges.len() as u32);
    Ok(CjDictionary {
        symbols: characters
            .iter()
            .map(|&character| character as u32)
            .collect(),
        first,
        edges,
        costs,
        target_bits,
    })
}
