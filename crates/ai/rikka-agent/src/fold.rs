use rikka_llm::{Content, Message, Request, Role, Usage};

use crate::compaction::{Compaction, Trigger};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::inner::Inner;
use crate::recall::{recalled, unloaded};
use crate::summary::{formatted, render_transcript, shortened, tag};

const SHORTENINGS: usize = 3;

const SHORTENED: &str = "[the start of this conversation is left out so that the rest fits]\n";

pub(crate) struct Fold {
    pub(crate) usage: Usage,
    pub(crate) messages_removed: usize,
}

impl Inner {
    pub(crate) async fn fold(
        &self,
        compaction: &Compaction,
        request: &mut Request,
        trigger: Trigger<'_>,
    ) -> Result<Option<Fold>> {
        let cut = compaction.cut(request.messages());
        if cut == 0 {
            return Ok(None);
        }
        self.check_cancelled()?;
        self.observe(Event::Compacting);

        let instruction = compaction.instruction(trigger);
        let (summary_text, usage) = self
            .summarise(
                compaction,
                request,
                cut,
                &instruction,
                trigger.shares_request(),
            )
            .await?;

        let (folded, retained) = request.messages().split_at(cut);
        let recalled = recalled(&self.functions, folded, retained);
        let summary =
            compaction.summary_message(&summary_text, recalled, unloaded(folded, retained));
        let lead = compaction.live(&summary, trigger);

        let mut retained = retained.to_vec();
        let lead_messages = match retained.first() {
            Some(first) if first.role() == Role::User => {
                let content = std::iter::once(Content::text(lead.as_str()))
                    .chain(first.content().iter().cloned())
                    .collect::<Vec<_>>();
                retained[0] = Message::new(Role::User, content);
                0
            }
            _ => {
                retained.insert(0, Message::user(lead.as_str()));
                1
            }
        };
        request.set_messages(retained);

        self.observe(Event::Compacted {
            summary: lead,
            compacted_messages: cut,
        });
        Ok(Some(Fold {
            usage,
            messages_removed: cut - lead_messages,
        }))
    }

    async fn summarise(
        &self,
        compaction: &Compaction,
        request: &Request,
        cut: usize,
        instruction: &str,
        sharing: bool,
    ) -> Result<(String, Usage)> {
        let mut usage = Usage::default();
        if sharing {
            match self.summarise_sharing(request, cut, instruction).await {
                Ok((summary, call_usage)) => {
                    usage += call_usage;
                    if !summary.trim().is_empty() {
                        return Ok((formatted(&summary), usage));
                    }
                }
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(_) => {}
            }
        }

        let (summary, call_usage) = self
            .summarise_transcript(compaction, &request.messages()[..cut], instruction)
            .await?;
        usage += call_usage;
        match summary.trim().is_empty() {
            true => Err(Error::Model(rikka_llm::Error::Malformed(
                "the summary the conversation was to be folded into was empty".to_owned(),
            ))),
            false => Ok((formatted(&summary), usage)),
        }
    }

    async fn summarise_sharing(
        &self,
        request: &Request,
        cut: usize,
        instruction: &str,
    ) -> Result<(String, Usage)> {
        let mut messages = request.messages()[..cut].to_vec();
        match messages.last_mut() {
            Some(last) if last.role() == Role::User => {
                let content = last
                    .content()
                    .iter()
                    .cloned()
                    .chain(std::iter::once(Content::text(instruction)))
                    .collect::<Vec<_>>();
                *last = Message::new(Role::User, content);
            }
            _ => messages.push(Message::user(instruction)),
        }

        let request = self.request(messages);
        let response = self.model.send(&self.shown(&request)).await?;
        let usage = response.usage();
        Ok((response.into_message().text(), usage))
    }

    async fn summarise_transcript(
        &self,
        compaction: &Compaction,
        folded: &[Message],
        instruction: &str,
    ) -> Result<(String, Usage)> {
        let mut messages = folded;
        let mut shortenings = 0;
        loop {
            let left_out = match messages.len() == folded.len() {
                true => "",
                false => SHORTENED,
            };
            let transcript = render_transcript(messages);
            let tag = tag(&transcript);
            let whole = format!("{instruction}\n\n<{tag}>\n{left_out}{transcript}\n</{tag}>");

            let mut chat = self
                .model
                .chat()
                .messages(vec![Message::user(whole)])
                .max_tokens(compaction.summary_tokens());
            if let Some(token) = self.cancel.clone() {
                chat = chat.cancel_token(token);
            }

            match self.model.send(&chat.build()).await {
                Ok(response) => {
                    let usage = response.usage();
                    return Ok((response.into_message().text(), usage));
                }
                Err(rikka_llm::Error::ContextOverflow) if shortenings < SHORTENINGS => {
                    let Some(rest) = shortened(messages) else {
                        return Err(Error::Model(rikka_llm::Error::ContextOverflow));
                    };
                    messages = rest;
                    shortenings += 1;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}
