use std::path::PathBuf;

use crate::change::{Change, State};
use crate::git;
use crate::stat::Stat;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    head: Option<String>,
    branch: Option<String>,
    upstream: Option<String>,
    tracking: bool,
    ahead: usize,
    behind: usize,
    born: bool,
    changes: Vec<Change>,
}

impl Status {
    #[must_use]
    pub fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    #[must_use]
    pub fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    #[must_use]
    pub fn upstream(&self) -> Option<&str> {
        self.upstream.as_deref()
    }

    #[must_use]
    pub fn is_upstream_gone(&self) -> bool {
        self.upstream.is_some() && !self.tracking
    }

    #[must_use]
    pub fn ahead(&self) -> usize {
        self.ahead
    }

    #[must_use]
    pub fn behind(&self) -> usize {
        self.behind
    }

    #[must_use]
    pub fn is_born(&self) -> bool {
        self.born
    }

    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    #[must_use]
    pub fn stat(&self) -> Stat {
        self.changes.iter().filter_map(Change::stat).sum()
    }

    pub(crate) fn count(&mut self, mut stat: impl FnMut(&Change) -> Option<Stat>) {
        for change in &mut self.changes {
            let counted = stat(change);
            change.count(counted);
        }
    }

    pub(crate) fn parse(output: &str) -> Status {
        let mut status = Status::default();
        let mut fields = git::fields(output);

        while let Some(field) = fields.next() {
            match field.as_bytes().first() {
                Some(b'#') => status.header(field),
                Some(b'1') => status.push(ordinary(field)),
                Some(b'2') => status.push(moved(field, fields.next())),
                Some(b'u') => status.push(unmerged(field)),
                Some(b'?') => status.push(untracked(field)),
                _ => {}
            }
        }

        status
            .changes
            .sort_by(|one, other| one.path().cmp(other.path()));
        status
    }

    fn push(&mut self, change: Option<Change>) {
        self.changes.extend(change);
    }

    fn header(&mut self, field: &str) {
        let Some((name, value)) = field
            .strip_prefix("# ")
            .and_then(|header| header.split_once(' '))
        else {
            return;
        };
        match name {
            "branch.oid" => {
                self.born = value != "(initial)";
                self.head = self.born.then(|| value.to_owned());
            }
            "branch.head" => self.branch = (value != "(detached)").then(|| value.to_owned()),
            "branch.upstream" => self.upstream = Some(value.to_owned()),
            "branch.ab" => {
                self.tracking = true;
                (self.ahead, self.behind) = divergence(value);
            }
            _ => {}
        }
    }
}

fn divergence(value: &str) -> (usize, usize) {
    let mut counts = value.split_whitespace().map(|count| {
        count
            .trim_start_matches(['+', '-'])
            .parse::<usize>()
            .unwrap_or_default()
    });
    (
        counts.next().unwrap_or_default(),
        counts.next().unwrap_or_default(),
    )
}

fn ordinary(field: &str) -> Option<Change> {
    let state = marks(field)?;
    let path = after(field, 8)?;
    Some(Change::new(PathBuf::from(path), None, state))
}

fn moved(field: &str, from: Option<&str>) -> Option<Change> {
    let state = marks(field)?;
    let path = after(field, 9)?;
    Some(Change::new(
        PathBuf::from(path),
        from.map(PathBuf::from),
        state,
    ))
}

fn unmerged(field: &str) -> Option<Change> {
    let path = after(field, 10)?;
    Some(Change::new(PathBuf::from(path), None, State::Conflicted))
}

fn untracked(field: &str) -> Option<Change> {
    let path = field.strip_prefix("? ").filter(|path| !path.is_empty())?;
    Some(Change::new(PathBuf::from(path), None, State::Untracked))
}

fn marks(field: &str) -> Option<State> {
    let mut marks = field.split(' ').nth(1)?.chars();
    Some(State::read(marks.next()?, marks.next()?))
}

fn after(field: &str, count: usize) -> Option<&str> {
    field
        .splitn(count + 1, ' ')
        .nth(count)
        .filter(|path| !path.is_empty())
}
