use std::fmt;
use std::sync::Arc;

use rikka_agent::{Deferred, Function, Reply, ToolKit, ToolKitRegistry};
use rikka_llm::{Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};

const SELECT: &str = "select:";

const NAMED: u32 = 10;
const PART: u32 = 5;
const DESCRIBED: u32 = 2;

#[derive(Debug, Deserialize)]
struct Args {
    query: String,
    #[serde(default)]
    max_results: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct ToolKitEntry {
    kit: ToolKit,
    first: bool,
}

impl ToolKitEntry {
    #[must_use]
    pub fn toolkit(&self) -> &ToolKit {
        &self.kit
    }

    #[must_use]
    pub fn is_first(&self) -> bool {
        self.first
    }
}

pub(crate) fn peek(deferred: &Deferred, kit: ToolKit) -> ToolKitEntry {
    ToolKitEntry {
        first: !deferred.is_introduced(kit.name()),
        kit,
    }
}

pub(crate) fn open(deferred: &Deferred, kit: ToolKit) -> ToolKitEntry {
    for function in kit.functions() {
        deferred.reveal(function.name());
    }
    ToolKitEntry {
        first: deferred.introduce(kit.name()),
        kit,
    }
}

struct Catalog<'a> {
    direct: &'a [Tool],
    kits: Arc<[ToolKit]>,
}

struct Found<'a> {
    tool: &'a Tool,
    kit: Option<&'a ToolKit>,
}

impl Catalog<'_> {
    fn every(&self) -> impl Iterator<Item = Found<'_>> {
        let direct = self.direct.iter().map(|tool| Found { tool, kit: None });
        let kit_tools = self.kits.iter().flat_map(|kit| {
            kit.functions().iter().map(move |function| Found {
                tool: function.tool(),
                kit: Some(kit),
            })
        });
        direct.chain(kit_tools)
    }

    fn kit(&self, name: &str) -> Option<&ToolKit> {
        self.kits
            .iter()
            .find(|kit| kit.name().eq_ignore_ascii_case(name))
    }

    fn names(&self) -> String {
        self.direct
            .iter()
            .map(|tool| tool.name().to_owned())
            .chain(
                self.kits
                    .iter()
                    .map(|kit| format!("{} (a kit)", kit.name())),
            )
            .collect::<Vec<String>>()
            .join(", ")
    }
}

pub(crate) fn declare(
    deferred: &Deferred,
    tools: Vec<Tool>,
    toolkits: Option<ToolKitRegistry>,
    prompt: &Prompt,
    facts: &Facts,
) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("query", prompt.param("query", facts)?)
        .integer("max_results", prompt.param("max_results", facts)?)
        .optional("max_results")
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let most = usize::try_from(prompt.limit("results")?).unwrap_or(usize::MAX);
    let tools: Arc<[Tool]> = tools.into();
    let deferred = deferred.clone();

    Ok(Function::new(tool, move |input: Value| {
        let catalog = Catalog {
            direct: &tools,
            kits: toolkits
                .as_ref()
                .map_or_else(|| Arc::from([]), ToolKitRegistry::kits),
        };
        let found = run(&deferred, &catalog, most, input);
        async move { found }
    }))
}

fn run(
    deferred: &Deferred,
    catalog: &Catalog<'_>,
    most: usize,
    input: Value,
) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let query = args.query.trim();
    if query.is_empty() {
        return Err(missing("there is nothing here to search for", catalog));
    }

    let most = args
        .max_results
        .map_or(most, |limit| usize::try_from(limit).unwrap_or(usize::MAX))
        .max(1);

    let found = match asked_for(query) {
        Some(names) => named_tools(catalog, names),
        None => searched(catalog, query, most),
    };

    if found.is_empty() {
        return Err(missing(&format!("nothing matches \"{query}\""), catalog));
    }

    let introduced: Vec<_> = found
        .iter()
        .filter_map(|found| found.kit)
        .filter(|kit| deferred.introduce(kit.name()) && !kit.instructions().trim().is_empty())
        .collect();
    let loaded: Vec<&str> = found.iter().map(|found| found.tool.name()).collect();
    let text = fmt::from_fn(|out| {
        for kit in &introduced {
            writeln!(
                out,
                "<instructions for=\"{}\">\n{}\n</instructions>",
                kit.name(),
                kit.instructions().trim()
            )?;
        }
        write!(
            out,
            "Loaded and callable from here on: {}",
            loaded.join(", ")
        )
    })
    .to_string();

    Ok(loaded
        .into_iter()
        .fold(Reply::new(text), |reply, tool| reply.load_tool(tool)))
}

fn asked_for(query: &str) -> Option<&str> {
    let (opening, names) = query.split_at_checked(SELECT.len())?;
    opening.eq_ignore_ascii_case(SELECT).then_some(names)
}

fn named_tools<'a>(catalog: &'a Catalog<'_>, names: &str) -> Vec<Found<'a>> {
    let mut found: Vec<Found<'a>> = Vec::new();

    for name in names.split(',').map(str::trim).filter(|it| !it.is_empty()) {
        let selected: Vec<Found<'a>> = match catalog.kit(name) {
            Some(kit) => kit
                .functions()
                .iter()
                .map(|function| Found {
                    tool: function.tool(),
                    kit: Some(kit),
                })
                .collect(),
            None => catalog
                .every()
                .filter(|found| found.tool.name().eq_ignore_ascii_case(name))
                .take(1)
                .collect(),
        };

        for selected in selected {
            if !found
                .iter()
                .any(|already| already.tool.name() == selected.tool.name())
            {
                found.push(selected);
            }
        }
    }

    found
}

fn searched<'a>(catalog: &'a Catalog<'_>, query: &str, most: usize) -> Vec<Found<'a>> {
    if let Some(found) = catalog
        .every()
        .find(|found| found.tool.name().eq_ignore_ascii_case(query))
    {
        return vec![found];
    }

    let query = query.to_lowercase();
    let mut needed = Vec::new();
    let mut selected = Vec::new();
    for term in query.split_whitespace() {
        match term.strip_prefix('+').filter(|term| !term.is_empty()) {
            Some(term) => needed.push(term),
            None => selected.push(term),
        }
    }

    let terms: Vec<&str> = needed.iter().chain(&selected).copied().collect();
    let mut scored: Vec<(u32, Found<'a>)> = catalog
        .every()
        .map(Indexed::new)
        .filter(|indexed| needed.iter().all(|term| indexed.reaches(term)))
        .map(|indexed| (indexed.score(&terms), indexed.found))
        .filter(|(score, _)| *score > 0)
        .collect();

    scored.sort_by(|(one, first), (two, second)| {
        two.cmp(one)
            .then_with(|| first.tool.name().cmp(second.tool.name()))
    });
    scored
        .into_iter()
        .take(most)
        .map(|(_, found)| found)
        .collect()
}

struct Indexed<'a> {
    found: Found<'a>,
    words: Vec<String>,
    description: String,
}

impl<'a> Indexed<'a> {
    fn new(found: Found<'a>) -> Indexed<'a> {
        let mut description = found.tool.description().to_lowercase();
        if let Some(kit) = found.kit {
            description.push(' ');
            description.push_str(&kit.description().to_lowercase());
        }
        Indexed {
            words: words(found.tool.name()),
            description,
            found,
        }
    }

    fn reaches(&self, term: &str) -> bool {
        self.words.iter().any(|word| word.contains(term)) || whole(&self.description, term)
    }

    fn score(&self, terms: &[&str]) -> u32 {
        terms.iter().map(|term| self.worth(term)).sum()
    }

    fn worth(&self, term: &str) -> u32 {
        let named = if self.words.iter().any(|word| word == term) {
            NAMED
        } else if self.words.iter().any(|word| word.contains(term)) {
            PART
        } else {
            0
        };
        named + u32::from(whole(&self.description, term)) * DESCRIBED
    }
}

fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut previous = None;
    let mut letters = name.chars().peekable();

    while let Some(letter) = letters.next() {
        if letter == '_' || letter == '-' || letter.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            previous = None;
            continue;
        }
        if !word.is_empty() && starts_word(letter, previous, letters.peek().copied()) {
            words.push(std::mem::take(&mut word));
        }
        word.extend(letter.to_lowercase());
        previous = Some(letter);
    }

    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn starts_word(letter: char, previous: Option<char>, next: Option<char>) -> bool {
    if !letter.is_uppercase() {
        return false;
    }
    match previous {
        Some(previous) if previous.is_uppercase() => next.is_some_and(char::is_lowercase),
        _ => true,
    }
}

fn whole(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + term.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

fn missing(reason: &str, catalog: &Catalog<'_>) -> String {
    format!("{reason}. The tools to search are: {}", catalog.names())
}
