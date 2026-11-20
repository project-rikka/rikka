use crate::cache::Cache;
use crate::content::Content;
use crate::message::Message;
use crate::request::Request;
use crate::role::Role;

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) tools: bool,
    pub(crate) system: bool,
    pub(crate) messages: Vec<usize>,
    pub(crate) automatic: bool,
}

impl Plan {
    pub(crate) fn marks(&self, message: usize) -> bool {
        self.messages.contains(&message)
    }
}

const LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Carries {
    Everything,
    Transcript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Blocks,
    Automatic,
}

pub(crate) fn plan(request: &Request, carries: Carries) -> Plan {
    place(request, carries, Placement::Blocks)
}

pub(crate) fn place(request: &Request, carries: Carries, placement: Placement) -> Plan {
    let mut plan = Plan::default();

    let mut room = LIMIT.saturating_sub(manual(request, carries));
    if request.cache == Cache::Off || room == 0 {
        return plan;
    }

    if carries == Carries::Everything {
        if !request.system.is_empty() {
            plan.system = true;
            room -= 1;
        } else if request.tools.iter().any(|tool| !tool.is_deferred())
            || !request.server_tools.is_empty()
        {
            plan.tools = true;
            room -= 1;
        }
    }

    if request.cache == Cache::Conversation {
        match placement {
            Placement::Blocks => {
                plan.messages = tail(&request.messages);
                plan.messages.truncate(room);
            }
            Placement::Automatic => plan.automatic = room > 0,
        }
    }

    plan
}

fn tail(messages: &[Message]) -> Vec<usize> {
    let Some(last) = messages
        .iter()
        .rposition(|message| message.role() != Role::System)
    else {
        return Vec::new();
    };

    let mut marks = vec![last];
    let earlier = messages[..last]
        .iter()
        .rposition(|message| message.role() == Role::User);

    if let Some(index) = earlier {
        marks.push(index);
    }

    marks
}

fn manual(request: &Request, carries: Carries) -> usize {
    let messages = request
        .messages
        .iter()
        .flat_map(|message| message.content())
        .filter(|content| Content::is_cached(content))
        .count();

    if carries == Carries::Transcript {
        return messages;
    }

    let system = request
        .system
        .iter()
        .filter(|text| text.is_cached())
        .count();
    let tools = request.tools.iter().filter(|tool| tool.is_cached()).count();

    system + tools + messages
}
