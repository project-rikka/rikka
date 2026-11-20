use std::time::{Duration, Instant};

use async_io::Timer;
use futures_util::StreamExt;
use futures_util::future::{Either, select};
use futures_util::stream::{SelectAll, select_all};
use zbus::blocking::Connection;
use zbus::message::Type;
use zbus::{MatchRule, Message, MessageStream, Task};

const MAX_QUEUED: usize = 64;

const DEBOUNCE_QUIET: Duration = Duration::from_millis(400);
const DEBOUNCE_MAX_DELAY: Duration = Duration::from_secs(2);

const TASK: &str = "rikka-bus-signals";

pub trait Coalesce: Send + 'static {
    fn merge(&mut self, other: Self);
}

impl Coalesce for () {
    fn merge(&mut self, (): ()) {}
}

#[derive(Clone, Debug, Default)]
pub struct Rule {
    sender: Option<String>,
    path: Option<PathMatch>,
    interface: Option<String>,
    member: Option<String>,
    argument: Option<(u8, String)>,
    namespace: Option<String>,
}

#[derive(Clone, Debug)]
enum PathMatch {
    Exact(String),
    Namespace(String),
}

impl Rule {
    #[must_use]
    pub fn new() -> Rule {
        Rule::default()
    }

    #[must_use]
    pub fn sender(mut self, sender: impl Into<String>) -> Rule {
        self.sender = Some(sender.into());
        self
    }

    #[must_use]
    pub fn interface(mut self, interface: impl Into<String>) -> Rule {
        self.interface = Some(interface.into());
        self
    }

    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Rule {
        self.path = Some(PathMatch::Exact(path.into()));
        self
    }

    #[must_use]
    pub fn path_namespace(mut self, namespace: impl Into<String>) -> Rule {
        self.path = Some(PathMatch::Namespace(namespace.into()));
        self
    }

    #[must_use]
    pub fn member(mut self, member: impl Into<String>) -> Rule {
        self.member = Some(member.into());
        self
    }

    #[must_use]
    pub fn arg(mut self, index: u8, value: impl Into<String>) -> Rule {
        self.argument = Some((index, value.into()));
        self
    }

    #[must_use]
    pub fn arg0_namespace(mut self, namespace: impl Into<String>) -> Rule {
        self.namespace = Some(namespace.into());
        self
    }

    fn into_match_rule(self) -> zbus::Result<MatchRule<'static>> {
        let mut rule = MatchRule::builder().msg_type(Type::Signal);
        if let Some(sender) = self.sender {
            rule = rule.sender(sender)?;
        }
        match self.path {
            Some(PathMatch::Exact(path)) => rule = rule.path(path)?,
            Some(PathMatch::Namespace(namespace)) => rule = rule.path_namespace(namespace)?,
            None => {}
        }
        if let Some(interface) = self.interface {
            rule = rule.interface(interface)?;
        }
        if let Some(member) = self.member {
            rule = rule.member(member)?;
        }
        if let Some((index, value)) = self.argument {
            rule = rule.arg(index, value)?;
        }
        if let Some(namespace) = self.namespace {
            rule = rule.arg0ns(namespace)?;
        }
        Ok(rule.build().into_owned())
    }
}

#[must_use]
#[derive(Debug)]
pub struct Watch(#[expect(dead_code, reason = "dropping the task ends the watch")] Task<()>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heard<T> {
    Signal(T),
    Closed,
}

#[derive(Clone, Debug)]
pub struct Signals {
    rules: Vec<Rule>,
    debounce: Debounce,
}

impl Default for Signals {
    fn default() -> Signals {
        Signals::new()
    }
}

impl Signals {
    #[must_use]
    pub fn new() -> Signals {
        Signals {
            rules: Vec::new(),
            debounce: Debounce {
                quiet: DEBOUNCE_QUIET,
                max_delay: DEBOUNCE_MAX_DELAY,
            },
        }
    }

    #[must_use]
    pub fn rule(mut self, rule: Rule) -> Signals {
        self.rules.push(rule);
        self
    }

    #[must_use]
    pub fn debounce(mut self, quiet: Duration, max_delay: Duration) -> Signals {
        self.debounce = Debounce { quiet, max_delay };
        self
    }

    pub fn listen<T: Coalesce>(
        self,
        connection: &Connection,
        parse: impl Fn(&Message) -> Option<T> + Send + 'static,
        deliver: impl Fn(T) -> bool + Send + 'static,
    ) -> zbus::Result<Watch> {
        let debounce = self.debounce;
        let mut signals = self.subscribe(connection)?;
        Ok(spawn(connection, async move {
            while let Some(message) = heard(&mut signals).await {
                let Some(mut pending) = parse(&message) else {
                    continue;
                };
                let started = Instant::now();
                let deadline = started + debounce.max_delay;
                let mut settled = started + debounce.quiet;
                loop {
                    let quiet = Timer::at(deadline.min(settled));
                    match select(std::pin::pin!(heard(&mut signals)), quiet).await {
                        Either::Left((Some(message), _)) => {
                            if let Some(next) = parse(&message) {
                                pending.merge(next);
                                settled = Instant::now() + debounce.quiet;
                            }
                        }
                        Either::Left((None, _)) => {
                            deliver(pending);
                            return;
                        }
                        Either::Right(_) => break,
                    }
                }
                if !deliver(pending) {
                    return;
                }
            }
        }))
    }

    pub fn each<T: Send + 'static>(
        self,
        connection: &Connection,
        parse: impl Fn(&Message) -> Option<T> + Send + 'static,
        deliver: impl Fn(T) -> bool + Send + 'static,
    ) -> zbus::Result<Watch> {
        self.hear(connection, parse, move |heard| match heard {
            Heard::Signal(signal) => deliver(signal),
            Heard::Closed => false,
        })
    }

    pub fn hear<T: Send + 'static>(
        self,
        connection: &Connection,
        parse: impl Fn(&Message) -> Option<T> + Send + 'static,
        deliver: impl Fn(Heard<T>) -> bool + Send + 'static,
    ) -> zbus::Result<Watch> {
        let mut signals = self.subscribe(connection)?;
        Ok(spawn(connection, async move {
            while let Some(message) = heard(&mut signals).await {
                if let Some(signal) = parse(&message)
                    && !deliver(Heard::Signal(signal))
                {
                    return;
                }
            }
            deliver(Heard::Closed);
        }))
    }

    fn subscribe(self, connection: &Connection) -> zbus::Result<SelectAll<MessageStream>> {
        let connection = connection.inner();
        let streams = self
            .rules
            .into_iter()
            .map(|rule| {
                async_io::block_on(MessageStream::for_match_rule(
                    rule.into_match_rule()?,
                    connection,
                    Some(MAX_QUEUED),
                ))
            })
            .collect::<zbus::Result<Vec<MessageStream>>>()?;
        Ok(select_all(streams))
    }
}

#[derive(Clone, Copy, Debug)]
struct Debounce {
    quiet: Duration,
    max_delay: Duration,
}

fn spawn(connection: &Connection, watching: impl Future<Output = ()> + Send + 'static) -> Watch {
    Watch(connection.inner().executor().spawn(watching, TASK))
}

async fn heard(signals: &mut SelectAll<MessageStream>) -> Option<Message> {
    loop {
        match signals.next().await? {
            Ok(message) => return Some(message),
            Err(error) => tracing::debug!("invalid bus signal: {error}"),
        }
    }
}
