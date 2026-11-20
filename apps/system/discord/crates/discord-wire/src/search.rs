use std::collections::HashMap;

use serde::Deserialize;

use crate::channel::Channel;
use crate::events::ThreadMember;
use crate::flex;
use crate::message::Message;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadSearch {
    #[serde(deserialize_with = "flex::list")]
    pub threads: Vec<Channel>,
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<ThreadMember>,
    #[serde(deserialize_with = "flex::flag")]
    pub has_more: bool,
    #[serde(deserialize_with = "flex::list")]
    pub first_messages: Vec<Message>,
    pub retry_after: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PostData {
    pub threads: HashMap<String, Post>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Post {
    pub first_message: Option<Message>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Created {
    #[serde(flatten)]
    pub thread: Channel,
    pub message: Option<Message>,
}
