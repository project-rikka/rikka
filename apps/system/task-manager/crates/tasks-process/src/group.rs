use std::collections::HashMap;

use crate::process::Process;

const DEPTH: usize = 64;

const MANAGERS: [&str; 4] = ["systemd", "init", "kthreadd", "rikka-compositor"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    App,
    Background,
    System,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Totals {
    pub cpu: f32,
    pub memory: Option<u64>,
    pub disk: f32,
    pub gpu: f32,
    pub threads: u32,
}

impl Totals {
    #[must_use]
    pub fn from_process(process: &Process) -> Self {
        Self {
            cpu: process.cpu,
            memory: process.memory,
            disk: process.disk,
            gpu: process.gpu,
            threads: process.threads,
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            cpu: self.cpu + other.cpu,
            memory: match (self.memory, other.memory) {
                (Some(left), Some(right)) => Some(left.saturating_add(right)),
                (left, right) => left.or(right),
            },
            disk: self.disk + other.disk,
            gpu: self.gpu + other.gpu,
            threads: self.threads + other.threads,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Group {
    leader: Process,
    members: Vec<Process>,
    category: Category,
    totals: Totals,
}

impl Group {
    #[must_use]
    pub fn category(&self) -> Category {
        self.category
    }

    #[must_use]
    pub fn leader(&self) -> &Process {
        &self.leader
    }

    #[must_use]
    pub fn members(&self) -> &[Process] {
        &self.members
    }

    pub fn processes(&self) -> impl Iterator<Item = &Process> {
        std::iter::once(&self.leader).chain(&self.members)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.members.len() + 1
    }

    #[must_use]
    pub fn totals(&self) -> Totals {
        self.totals
    }

    fn totalled(mut self) -> Self {
        self.totals = self
            .processes()
            .map(Totals::from_process)
            .fold(Totals::default(), Totals::add);
        self
    }
}

pub(crate) struct Member {
    pub(crate) process: Process,
    pub(crate) program: String,
    pub(crate) application: bool,
    pub(crate) system: bool,
}

pub(crate) fn assemble(members: Vec<Member>) -> Vec<Group> {
    let index: HashMap<i32, usize> = members
        .iter()
        .enumerate()
        .map(|(position, member)| (member.process.pid, position))
        .collect();
    let leaders: Vec<bool> = members
        .iter()
        .map(|member| leads(member, &members, &index))
        .collect();
    let tree: Vec<(i32, i32)> = members
        .iter()
        .map(|member| (member.process.pid, member.process.parent))
        .collect();
    let categories: Vec<Category> = members.iter().map(category).collect();

    let mut groups: HashMap<i32, Group> = HashMap::new();
    let mut order: Vec<i32> = Vec::new();
    let mut unassigned: Vec<(usize, Process)> = Vec::new();
    for (position, member) in members.into_iter().enumerate() {
        if leaders[position] {
            order.push(member.process.pid);
            groups.insert(
                member.process.pid,
                Group {
                    leader: member.process,
                    members: Vec::new(),
                    category: categories[position],
                    totals: Totals::default(),
                },
            );
        } else {
            unassigned.push((position, member.process));
        }
    }
    for (position, process) in unassigned {
        let Some(leader) = leader_of(position, &tree, &index, &leaders) else {
            continue;
        };
        if let Some(group) = groups.get_mut(&tree[leader].0) {
            group.members.push(process);
        }
    }

    order
        .into_iter()
        .filter_map(|pid| groups.remove(&pid))
        .map(Group::totalled)
        .collect()
}

fn category(member: &Member) -> Category {
    if member.system {
        Category::System
    } else if member.application {
        Category::App
    } else {
        Category::Background
    }
}

fn leads(member: &Member, members: &[Member], index: &HashMap<i32, usize>) -> bool {
    let Some(parent) = index.get(&member.process.parent).map(|at| &members[*at]) else {
        return true;
    };
    if member.process.parent <= 2 || MANAGERS.contains(&parent.program.as_str()) {
        return true;
    }
    if parent.system != member.system {
        return true;
    }
    member.application && parent.program != member.program
}

fn leader_of(
    position: usize,
    tree: &[(i32, i32)],
    index: &HashMap<i32, usize>,
    leaders: &[bool],
) -> Option<usize> {
    let mut at = position;
    for _ in 0..DEPTH {
        let parent = *index.get(&tree[at].1)?;
        if leaders[parent] {
            return Some(parent);
        }
        at = parent;
    }
    None
}
