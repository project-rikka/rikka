#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Fatal,
}

impl Level {
    fn parse(text: &str) -> Option<Level> {
        Some(match text {
            "TRACE" => Level::Trace,
            "DEBUG" => Level::Debug,
            "INFO" => Level::Info,
            "WARN" | "WARNING" => Level::Warn,
            "ERROR" => Level::Error,
            "FATAL" => Level::Fatal,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub level: Level,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LevelTracker {
    last: Level,
}

impl LevelTracker {
    pub fn line(&mut self, text: String) -> LogLine {
        let level = header_level(&text).unwrap_or(self.last);
        self.last = level;
        LogLine { level, text }
    }
}

fn header_level(line: &str) -> Option<Level> {
    let rest = line.strip_prefix('[')?;
    let (_, rest) = rest.split_once("] [")?;
    let (thread, _) = rest.split_once(']')?;
    let (_, level) = thread.rsplit_once('/')?;
    Level::parse(level)
}
