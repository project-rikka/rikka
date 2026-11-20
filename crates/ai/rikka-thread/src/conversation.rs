use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::{Stream, StreamExt};
use rikka_agent::{
    Agent, Error as AgentError, Event, Interjections, Limit, Notice, Outcome, Session,
};
use rikka_llm::{Content, Image, ImageSource, Stop, Trouble};
use rikka_llm_accounts::ConnectedModel;
use rikka_types::CancelToken;

use crate::ask::Ask;
use crate::assistant::Assistant;
use crate::attachment::Attachment;
use crate::call::CallResult;
use crate::error::Result;
use crate::instructing::Instructing;
use crate::replay::Replay;
use crate::reply::Reply;
use crate::run_context::RunContext;
use crate::thread::{Thread, now};
use crate::turn::Speaker;

#[derive(Debug)]
pub(crate) struct Conversation {
    session: Session,
    model: String,
    work: Work,
    cancel: CancelToken,
    interjections: Interjections,
    lead: Option<String>,
    replies: UnboundedReceiver<Reply>,
}

#[derive(Debug)]
enum Work {
    Ask(Vec<Content>),
    Compact(String),
}

enum Step {
    Running(Pin<Box<dyn Future<Output = Reply> + Send>>),
    Ending(Reply),
    Ended,
}

impl Conversation {
    pub(crate) fn new(
        assistant: &Assistant,
        hired: ConnectedModel,
        thread: &mut Thread,
        ask: impl Into<Ask>,
    ) -> Result<Conversation> {
        Conversation::asking(assistant, hired, thread, Speaker::User, ask.into())
    }

    pub(crate) fn compacting(
        assistant: &Assistant,
        hired: ConnectedModel,
        thread: &Thread,
        focus: &str,
    ) -> Result<Conversation> {
        let sent = Instructing::sent(assistant, thread, hired.name());
        let work = Work::Compact(focus.to_owned());
        Conversation::open(assistant, hired, thread, &sent, work)
    }

    pub(crate) fn woken(
        assistant: &Assistant,
        hired: ConnectedModel,
        thread: &mut Thread,
    ) -> Result<Option<Conversation>> {
        let mailbox = assistant.notices();
        let notices = mailbox.take();
        if notices.is_empty() {
            return Ok(None);
        }

        let notice = Ask::new(joined(&notices));
        match Conversation::asking(assistant, hired, thread, Speaker::Notice, notice) {
            Ok(conversation) => Ok(Some(conversation)),
            Err(error) => {
                for notice in notices {
                    mailbox.post(notice);
                }
                Err(error)
            }
        }
    }

    fn asking(
        assistant: &Assistant,
        hired: ConnectedModel,
        thread: &mut Thread,
        speaker: Speaker,
        ask: Ask,
    ) -> Result<Conversation> {
        let instructing = Instructing::new(assistant, thread, hired.name());
        let ask = ask.led_by(instructing.restated);
        let work = Work::Ask(ask.content());
        let conversation = Conversation::open(assistant, hired, thread, &instructing.sent, work)?;
        let at = thread.add_request(speaker, ask, instructing.recorded);
        if let Some(history) = assistant.history() {
            history.set_checkpoint(at);
        }
        Ok(conversation)
    }

    fn open(
        assistant: &Assistant,
        hired: ConnectedModel,
        thread: &Thread,
        instructions: &[String],
        work: Work,
    ) -> Result<Conversation> {
        let cancel = CancelToken::new();
        let interjections = Interjections::new();
        let named = hired.model().name().to_owned();
        let (sender, replies) = unbounded();

        let run_context = RunContext {
            hired,
            key: thread.id().to_owned(),
            cancel: cancel.clone(),
        };

        let history = assistant.history();
        let observer = move |event: Event| {
            let Some(reply) = reply_for(event) else {
                return;
            };
            if let (Reply::Interjected { at, .. }, Some(history)) = (&reply, &history) {
                history.set_checkpoint(*at);
            }
            let _sent = sender.unbounded_send(reply);
        };
        let agent: Agent = assistant.agent(&run_context, instructions, &interjections, observer)?;
        if let Some(subagents) = assistant.subagents() {
            subagents.set_run_context(run_context);
        }
        assistant.questions().reopen();

        Ok(Conversation {
            session: agent
                .resume(thread.transcript(&named))
                .filled(thread.context_tokens()),
            model: named,
            work,
            cancel,
            interjections,
            lead: assistant.interjection_lead().map(str::to_owned),
            replies,
        })
    }

    #[must_use]
    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub(crate) fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    pub(crate) fn interjections(&self) -> &Interjections {
        &self.interjections
    }

    pub(crate) fn lead(&self) -> Option<&str> {
        self.lead.as_deref()
    }

    pub(crate) fn replies(self) -> impl Stream<Item = Reply> + Send {
        let Conversation {
            mut session,
            work,
            mut replies,
            ..
        } = self;

        let run = Box::pin(async move {
            let ended = match work {
                Work::Ask(content) => session
                    .send(content)
                    .await
                    .map(|outcome| cut_short(&outcome)),
                Work::Compact(focus) => session.compact(&focus).await.map(|_| None),
            };
            match ended {
                Ok(Some(reason)) => Reply::Failed(Trouble::new(reason)),
                Ok(None) => Reply::Done,
                Err(AgentError::Cancelled) => Reply::Stopped,
                Err(refused) => Reply::Failed(Trouble::from(&refused)),
            }
        });

        let mut step = Step::Running(run);
        futures_util::stream::poll_fn(move |context| {
            loop {
                match std::mem::replace(&mut step, Step::Ended) {
                    Step::Running(mut run) => {
                        if let Poll::Ready(Some(reply)) = replies.poll_next_unpin(context) {
                            step = Step::Running(run);
                            return Poll::Ready(Some(reply));
                        }
                        match run.as_mut().poll(context) {
                            Poll::Ready(ended) => step = Step::Ending(ended),
                            Poll::Pending => {
                                step = Step::Running(run);
                                return Poll::Pending;
                            }
                        }
                    }

                    Step::Ending(ended) => {
                        if let Ok(reply) = replies.try_recv() {
                            step = Step::Ending(ended);
                            return Poll::Ready(Some(reply));
                        }
                        return Poll::Ready(Some(ended));
                    }

                    Step::Ended => return Poll::Ready(None),
                }
            }
        })
    }
}

fn cut_short(outcome: &Outcome) -> Option<String> {
    if outcome.exhausted() == Some(Limit::Output) {
        return Some(format!("the answer {}", Limit::Output.reason()));
    }

    match outcome.stop() {
        Stop::Declined(reason) => Some(format!("the model declined to answer: {reason}")),
        Stop::Filtered => Some("the answer was filtered before it reached here".to_owned()),
        _ => None,
    }
}

pub(crate) fn reply_for(event: Event) -> Option<Reply> {
    match event {
        Event::Text(text) => Some(Reply::Delta(text)),
        Event::Responded(message) => Some(Reply::Responded(Replay::from_message(message))),
        Event::Hosted(part) => Some(Reply::Hosted(part)),
        Event::TurnFinished { usage, .. } => Some(Reply::TurnFinished(usage)),
        Event::Compacting => Some(Reply::Compacting),
        Event::Compacted { summary, .. } => Some(Reply::Compacted(summary)),
        Event::CompactFailed { reason } => Some(Reply::CompactFailed(reason)),
        Event::Resumed => Some(Reply::Resumed),
        Event::Noticed { text } => Some(Reply::Noticed(text)),
        Event::Interjected { id } => Some(Reply::Interjected { id, at: now() }),
        Event::ToolStarted { id, name, input } => Some(Reply::Started {
            id: Arc::from(id),
            tool: Arc::from(name),
            input: Arc::new(input),
        }),
        Event::ToolFinished {
            id,
            name,
            output,
            failed,
            detail,
            images,
            displayed,
            loaded,
        } => Some(Reply::Finished {
            id: Arc::from(id),
            result: CallResult {
                output: Arc::from(output),
                failed,
                detail: detail
                    .and_then(|detail| serde_json::from_value(detail).ok())
                    .map(Arc::new),
                pictured: pictures(&name, &images),
                displayed: pictures(&name, &displayed),
                loaded: loaded.into_iter().map(Arc::from).collect(),
            },
        }),
        _ => None,
    }
}

fn pictures(tool: &str, images: &[Image]) -> Vec<Attachment> {
    images
        .iter()
        .filter_map(|image| match image.source() {
            ImageSource::Inline { media, data } => {
                match Attachment::new(tool, *media, Arc::clone(data)) {
                    Ok(picture) => Some(picture),
                    Err(error) => {
                        tracing::warn!("failed to store image from {tool}: {error}");
                        None
                    }
                }
            }
            ImageSource::Url(_) => None,
        })
        .collect()
}

pub(crate) fn joined(notices: &[Notice]) -> String {
    notices
        .iter()
        .map(Notice::text)
        .collect::<Vec<&str>>()
        .join("\n")
}
