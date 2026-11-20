#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Sleeping,
    Waiting,
    Stopped,
    Zombie,
    Idle,
}

impl State {
    pub(crate) fn parse(state: &str) -> Self {
        match state.chars().next() {
            Some('R') => Self::Running,
            Some('D') => Self::Waiting,
            Some('T' | 't') => Self::Stopped,
            Some('Z' | 'X') => Self::Zombie,
            Some('I') => Self::Idle,
            _ => Self::Sleeping,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: i32,
    pub start_time: u64,
    pub parent: i32,
    pub name: String,
    pub command: String,
    pub user: String,
    pub state: State,
    pub cpu: f32,
    pub memory: Option<u64>,
    pub disk: f32,
    pub gpu: f32,
    pub threads: u32,
    pub(crate) search_text: String,
}

impl Process {
    #[must_use]
    pub fn is_kernel(&self) -> bool {
        self.command.is_empty()
    }

    #[must_use]
    pub fn matches_search(&self, lowercase: &str) -> bool {
        self.search_text.contains(lowercase)
    }
}

pub(crate) fn search_text(name: &str, command: &str) -> String {
    let mut search_text = name.to_lowercase();
    search_text.push('\n');
    search_text.push_str(&command.to_lowercase());
    search_text
}
