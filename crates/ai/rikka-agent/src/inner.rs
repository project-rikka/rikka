use std::borrow::Cow;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use futures_util::future::join_all;
use rikka_llm::{
    Cache, CancelToken, Content, Effort, Event as ModelEvent, Format, Message, Model, Request,
    Response, Reveal, ServerTool, Stop, Text, Tool, ToolResult, ToolUse, Ttl, Usage,
};

use crate::budget::{Budget, Limit};
use crate::compaction::{Compaction, Trigger};
use crate::deferred::Deferred;
use crate::error::{Error, Result};
use crate::event::{Event, Observer};
use crate::fold::Fold;
use crate::gauge::Gauge;
use crate::interjections::Interjections;
use crate::notices::{Notice, Notices};
use crate::outcome::Outcome;
use rikka_tool::Function;
use rikka_tool::{ToolKit, ToolKitRegistry};

const RESUME_LIMIT: u32 = 3;

const REFUSALS: u32 = 3;

const UNSEEN: &str = include_str!("../prompts/unseen.md");

const STOPPED: &str = "the run was stopped while this call was running";

const NEVER_RUN: &str = "the run was stopped before this call was made";

const CARRY_ON: &str = "That answer was cut short by the output limit. Carry straight on from \
     where it stopped — no apology, no recap — and take what is left in smaller pieces.";

pub(crate) struct Inner {
    pub(crate) model: Model,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) instructions: Vec<Text>,
    pub(crate) functions: Vec<Function>,
    pub(crate) server_tools: Vec<ServerTool>,
    pub(crate) deferred: Deferred,
    pub(crate) rack: Option<ToolKitRegistry>,
    pub(crate) budget: Budget,
    pub(crate) compaction: Option<Compaction>,
    pub(crate) effort: Option<Effort>,
    pub(crate) reveal: Reveal,
    pub(crate) format: Option<Format>,
    pub(crate) cache: Cache,
    pub(crate) ttl: Ttl,
    pub(crate) cache_key: Option<String>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) temperature: Option<f32>,
    pub(crate) parallel_tools: bool,
    pub(crate) observer: Option<Arc<dyn Observer>>,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) notices: Notices,
    pub(crate) waits: bool,
    pub(crate) interjections: Interjections,
    pub(crate) vision: bool,
}

impl Inner {
    pub(crate) async fn run(
        &self,
        request: &mut Request,
        mut from: usize,
        gauge: &mut Gauge,
    ) -> Result<Outcome> {
        let mut usage = Usage::default();
        let mut turns = 0;
        let mut resumes = 0;
        let mut carried = String::new();
        let mut stop = Stop::End;
        let mut refusals = 0;
        let mut overflowed = false;

        let exhausted = loop {
            self.check_cancelled()?;
            self.redeclare(request);
            if let Some(limit) = self.budget.reached(turns, usage) {
                break Some(limit);
            }

            if let Some(folded) = self
                .fold_when(request, gauge, &mut refusals, Trigger::Automatic)
                .await?
            {
                usage += folded.usage;
                from = from.saturating_sub(folded.messages_removed);
            }

            let response = match self.answer(request).await {
                Ok(response) => response,
                Err(Error::Model(rikka_llm::Error::ContextOverflow)) if !overflowed => {
                    overflowed = true;
                    match self
                        .fold_when(request, gauge, &mut refusals, Trigger::Overflowed)
                        .await?
                    {
                        Some(folded) => {
                            usage += folded.usage;
                            from = from.saturating_sub(folded.messages_removed);
                            continue;
                        }
                        None => return Err(Error::Model(rikka_llm::Error::ContextOverflow)),
                    }
                }
                Err(error) => return Err(error),
            };
            overflowed = false;
            turns += 1;
            let turn_usage = response.usage();
            usage += turn_usage;
            stop = response.stop().clone();

            let answer = response.into_message();
            let calls = answer.tool_calls().cloned().collect::<Vec<_>>();
            let responded = self.observer.is_some().then(|| answer.clone());
            request.push(answer);
            *gauge = Gauge::counted(turn_usage.total(), request.messages().len());
            self.observe(Event::TurnFinished {
                turn: turns,
                usage: turn_usage,
                total_usage: usage,
            });
            if let Some(responded) = responded {
                self.observe(Event::Responded(responded));
            }

            if calls.is_empty() {
                if stop.is_paused() {
                    continue;
                } else if stop != Stop::MaxTokens {
                    match self.deliver_notices(request).await? {
                        Some(notice_usage) => usage += notice_usage,
                        None => break None,
                    }
                } else if resumes >= RESUME_LIMIT {
                    break Some(Limit::Output);
                } else {
                    resumes += 1;
                    carried.push_str(&request_text(request));
                    request.push(Message::user(CARRY_ON));
                    self.observe(Event::Resumed);
                }
            } else {
                if self.is_cancelled() {
                    request.push(cancelled(&calls));
                    return Err(Error::Cancelled);
                }

                let (results, ran) = self.run_tools(calls).await;
                usage += ran;
                let mut results_message = Message::tool_results(results);
                if let Some((text, notice_usage)) = self.notice(self.notices.take()) {
                    usage += notice_usage;
                    results_message = results_message.with(Content::text(text));
                }
                request.push(results_message);
                self.interject(request);
            }
        };

        Ok(Outcome::new(
            request.messages()[from..].to_vec(),
            usage,
            turns,
            stop,
            exhausted,
            carried,
        ))
    }

    pub(crate) async fn compact(&self, request: &mut Request, focus: &str) -> Result<Usage> {
        let compaction = self.compaction.as_ref().ok_or(Error::Unfoldable)?;
        match self
            .fold(compaction, request, Trigger::Asked(focus))
            .await?
        {
            Some(folded) => Ok(folded.usage),
            None => Err(Error::NothingToFold),
        }
    }

    async fn fold_when(
        &self,
        request: &mut Request,
        gauge: &mut Gauge,
        refusals: &mut u32,
        trigger: Trigger<'_>,
    ) -> Result<Option<Fold>> {
        let Some(compaction) = self.compaction.as_ref() else {
            return Ok(None);
        };
        let due = *refusals < REFUSALS && compaction.is_full(gauge.reading(request));
        if trigger != Trigger::Overflowed && !due {
            return Ok(None);
        }

        match self.fold(compaction, request, trigger).await {
            Ok(Some(folded)) => {
                *refusals = 0;
                *gauge = Gauge::default();
                Ok(Some(folded))
            }
            Ok(None) => Ok(None),
            Err(Error::Cancelled) => Err(Error::Cancelled),
            Err(error) => {
                *refusals += 1;
                self.observe(Event::CompactFailed {
                    reason: error.to_string(),
                });
                Ok(None)
            }
        }
    }

    async fn deliver_notices(&self, request: &mut Request) -> Result<Option<Usage>> {
        let usage = match self.notice(self.notices.take()) {
            Some((text, usage)) => {
                request.push(Message::user(text));
                Some(usage)
            }
            None => None,
        };
        let interjected = self.interject(request);
        if usage.is_some() || interjected {
            return Ok(Some(usage.unwrap_or_default()));
        }
        if !self.waits || self.notices.outstanding() == 0 {
            return Ok(None);
        }

        let Some(arrived) = self.unless_stopped(self.notices.wait()).await else {
            return Err(Error::Cancelled);
        };

        match self.notice(arrived) {
            Some((text, usage)) => {
                request.push(Message::user(text));
                Ok(Some(usage))
            }
            None => Ok(None),
        }
    }

    fn notice(&self, arrived: Vec<Notice>) -> Option<(String, Usage)> {
        if arrived.is_empty() {
            return None;
        }

        let mut usage = Usage::default();
        let mut text = Vec::with_capacity(arrived.len());
        for notice in arrived {
            usage += notice.usage();
            self.observe(Event::Noticed {
                text: notice.text().to_owned(),
            });
            text.push(notice.text().to_owned());
        }

        Some((text.join("\n"), usage))
    }

    fn interject(&self, request: &mut Request) -> bool {
        if self.is_cancelled() {
            return false;
        }
        let interjections = self.interjections.take();
        for interjection in &interjections {
            self.observe(Event::Interjected {
                id: interjection.id(),
            });
            request.push(Message::user(interjection.content().to_vec()));
        }
        !interjections.is_empty()
    }

    pub(crate) fn request(&self, history: Vec<Message>) -> Request {
        let mut chat = self
            .model
            .chat()
            .messages(history)
            .tools(self.declared())
            .server_tools(self.server_tools.iter().copied())
            .parallel_tools(self.parallel_tools)
            .reveal(self.reveal)
            .cache(self.cache)
            .ttl(self.ttl);

        if let Some(key) = self.cache_key.clone() {
            chat = chat.cache_key(key);
        }

        for instruction in &self.instructions {
            chat = chat.system(instruction.clone());
        }
        if let Some(effort) = self.effort {
            chat = chat.effort(effort);
        }
        if let Some(format) = self.format.clone() {
            chat = chat.format(format);
        }
        if let Some(tokens) = self.max_tokens {
            chat = chat.max_tokens(tokens);
        }
        if let Some(temperature) = self.temperature {
            chat = chat.temperature(temperature);
        }
        if let Some(token) = self.cancel.clone() {
            chat = chat.cancel_token(token);
        }

        chat.build()
    }

    pub(crate) fn shown<'a>(&self, request: &'a Request) -> Cow<'a, Request> {
        if self.vision || !request.messages().iter().any(Message::has_images) {
            return Cow::Borrowed(request);
        }
        let placeholder = UNSEEN.trim();
        let mut shown = request.clone();
        shown.set_messages(
            request
                .messages()
                .iter()
                .map(|message| message.replacing_images(placeholder))
                .collect(),
        );
        Cow::Owned(shown)
    }

    fn declared(&self) -> Vec<Tool> {
        let kits = self.kits();
        self.offering(&kits)
            .into_iter()
            .map(|(function, deferred)| declared_tool(function, deferred))
            .collect()
    }

    fn offering<'a>(&'a self, kits: &'a [ToolKit]) -> Vec<(&'a Function, bool)> {
        let mut offering: Vec<(&Function, bool)> = self
            .functions
            .iter()
            .map(|function| (function, self.deferred.is_hidden(function.name())))
            .collect();
        let mut named: HashSet<&str> = self.functions.iter().map(Function::name).collect();

        for function in kits.iter().flat_map(ToolKit::functions) {
            if named.insert(function.name()) {
                offering.push((function, !self.deferred.is_revealed(function.name())));
            }
        }

        offering
    }

    fn kits(&self) -> Arc<[ToolKit]> {
        self.rack
            .as_ref()
            .map_or_else(|| Arc::from([]), ToolKitRegistry::kits)
    }

    fn redeclare(&self, request: &mut Request) {
        if self.deferred.is_empty() && self.rack.is_none() {
            return;
        }

        let kits = self.kits();
        let offering = self.offering(&kits);
        let same = offering.len() == request.tools().len()
            && offering
                .iter()
                .zip(request.tools())
                .all(|((function, deferred), tool)| {
                    function.name() == tool.name() && *deferred == tool.is_deferred()
                });
        if !same {
            request.set_tools(self.declared());
        }
    }

    async fn answer(&self, request: &Request) -> Result<Response> {
        let mut stream = self.model.stream(&self.shown(request)).await?;

        while let Some(event) = stream.next().await {
            match event? {
                ModelEvent::Text(text) => self.observe(Event::Text(text)),
                ModelEvent::Reasoning(text) => self.observe(Event::Reasoning(text)),
                ModelEvent::Hosted(part) => self.observe(Event::Hosted(part)),
                ModelEvent::Done(response) => return Ok(*response),
                _ => {}
            }
        }

        Err(Error::Model(rikka_llm::Error::Malformed(
            "the answer ended before the model did".to_owned(),
        )))
    }

    async fn run_tools(&self, calls: Vec<ToolUse>) -> (Vec<ToolResult>, Usage) {
        let answers = join_all(calls.into_iter().map(|call| self.run_tool(call))).await;

        let mut usage = Usage::default();
        let mut results = Vec::with_capacity(answers.len());
        for (result, call_usage) in answers {
            usage += call_usage;
            results.push(result);
        }

        (results, usage)
    }

    async fn run_tool(&self, call: ToolUse) -> (ToolResult, Usage) {
        self.observe(Event::ToolStarted {
            id: call.id().to_owned(),
            name: call.name().to_owned(),
            input: call.input().clone(),
        });

        let Some(function) = self.find(call.name()) else {
            let reason = format!("there is no tool named \"{}\"", call.name());
            return (self.failed(&call, reason), Usage::default());
        };

        match self
            .unless_stopped(function.call(call.input().clone()))
            .await
        {
            None => (self.failed(&call, STOPPED.to_owned()), Usage::default()),
            Some(Ok(reply)) => {
                let usage = reply.usage();
                let parts = reply.into_parts();

                let mut result = match parts.failed {
                    true => ToolResult::failed(call.id(), parts.text.clone()),
                    false => ToolResult::new(call.id(), parts.text.clone()),
                };
                for image in parts.images.iter().chain(&parts.displayed) {
                    result = result.image(image.clone());
                }
                for tool in &parts.loaded_tools {
                    result = result.reference(tool.clone());
                }

                self.observe(Event::ToolFinished {
                    id: call.id().to_owned(),
                    name: call.name().to_owned(),
                    output: parts.text,
                    failed: parts.failed,
                    detail: parts.detail,
                    images: parts.images,
                    displayed: parts.displayed,
                    loaded: parts.loaded_tools,
                });
                (result, usage)
            }
            Some(Err(reason)) => (self.failed(&call, reason), Usage::default()),
        }
    }

    async fn unless_stopped<T>(&self, work: impl Future<Output = T>) -> Option<T> {
        let Some(token) = self.cancel.as_ref() else {
            return Some(work.await);
        };

        tokio::select! {
            biased;
            done = work => Some(done),
            () = token.cancelled() => None,
        }
    }

    fn failed(&self, call: &ToolUse, reason: String) -> ToolResult {
        self.observe(Event::ToolFinished {
            id: call.id().to_owned(),
            name: call.name().to_owned(),
            output: reason.clone(),
            failed: true,
            detail: None,
            images: Vec::new(),
            displayed: Vec::new(),
            loaded: Vec::new(),
        });
        ToolResult::failed(call.id(), reason)
    }

    fn find(&self, name: &str) -> Option<Function> {
        self.functions
            .iter()
            .find(|function| function.name() == name)
            .cloned()
            .or_else(|| self.rack.as_ref()?.function(name))
    }

    pub(crate) fn observe(&self, event: Event) {
        if let Some(observer) = &self.observer {
            observer.observe(event);
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(CancelToken::is_cancelled)
    }

    pub(crate) fn check_cancelled(&self) -> Result<()> {
        match self.is_cancelled() {
            true => Err(Error::Cancelled),
            false => Ok(()),
        }
    }
}

fn declared_tool(function: &Function, deferred: bool) -> Tool {
    let tool = function.tool().clone();
    match deferred {
        true => tool.deferred(),
        false => tool,
    }
}
fn request_text(request: &Request) -> String {
    request
        .messages()
        .last()
        .map(Message::text)
        .unwrap_or_default()
}

fn cancelled(calls: &[ToolUse]) -> Message {
    Message::tool_results(
        calls
            .iter()
            .map(|call| ToolResult::failed(call.id(), NEVER_RUN)),
    )
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Agent")
            .field("name", &self.name)
            .field("model", &self.model.name())
            .field("tools", &self.functions.len())
            .finish_non_exhaustive()
    }
}
