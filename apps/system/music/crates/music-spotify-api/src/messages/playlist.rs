#[derive(Clone, PartialEq, prost::Message)]
pub struct SelectedListContent {
    #[prost(int32, tag = "2")]
    pub length: i32,
    #[prost(message, optional, tag = "3")]
    pub attributes: Option<ListAttributes>,
    #[prost(message, optional, tag = "5")]
    pub contents: Option<ListItems>,
    #[prost(string, tag = "16")]
    pub owner_username: String,
    #[prost(message, optional, tag = "18")]
    pub capabilities: Option<Capabilities>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListItems {
    #[prost(int32, tag = "1")]
    pub pos: i32,
    #[prost(bool, tag = "2")]
    pub truncated: bool,
    #[prost(message, repeated, tag = "3")]
    pub items: Vec<Item>,
    #[prost(message, repeated, tag = "4")]
    pub meta_items: Vec<MetaItem>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Item {
    #[prost(string, required, tag = "1")]
    pub uri: String,
    #[prost(message, optional, tag = "2")]
    pub attributes: Option<ItemAttributes>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ItemAttributes {
    #[prost(int64, optional, tag = "2")]
    pub timestamp: Option<i64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MetaItem {
    #[prost(message, optional, tag = "2")]
    pub attributes: Option<ListAttributes>,
    #[prost(int32, tag = "3")]
    pub length: i32,
    #[prost(string, tag = "5")]
    pub owner_username: String,
    #[prost(message, optional, tag = "7")]
    pub capabilities: Option<Capabilities>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ListAttributes {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub description: Option<String>,
    #[prost(bytes = "vec", tag = "3")]
    pub picture: Vec<u8>,
    #[prost(bool, tag = "4")]
    pub collaborative: bool,
    #[prost(message, repeated, tag = "13")]
    pub picture_size: Vec<PictureSize>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct PictureSize {
    #[prost(string, tag = "1")]
    pub target_name: String,
    #[prost(string, tag = "2")]
    pub url: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Capabilities {
    #[prost(bool, optional, tag = "5")]
    pub can_edit_items: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CreateListReply {
    #[prost(string, required, tag = "1")]
    pub(crate) uri: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct OpList {
    #[prost(message, repeated, tag = "1")]
    pub(crate) ops: Vec<Op>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Op {
    #[prost(int32, required, tag = "1")]
    pub(crate) kind: i32,
    #[prost(message, optional, tag = "2")]
    pub(crate) add: Option<Add>,
    #[prost(message, optional, tag = "3")]
    pub(crate) rem: Option<Rem>,
    #[prost(message, optional, tag = "6")]
    pub(crate) update_list_attributes: Option<UpdateListAttributes>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Add {
    #[prost(message, repeated, tag = "2")]
    pub(crate) items: Vec<Item>,
    #[prost(bool, optional, tag = "4")]
    pub(crate) add_last: Option<bool>,
    #[prost(bool, optional, tag = "5")]
    pub(crate) add_first: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Rem {
    #[prost(message, repeated, tag = "3")]
    pub(crate) items: Vec<Item>,
    #[prost(bool, optional, tag = "7")]
    pub(crate) items_as_key: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct UpdateListAttributes {
    #[prost(message, optional, tag = "1")]
    pub(crate) new_attributes: Option<ListAttributesPartialState>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ListAttributesPartialState {
    #[prost(message, optional, tag = "1")]
    pub(crate) values: Option<ListAttributes>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ListChanges {
    #[prost(message, repeated, tag = "2")]
    pub(crate) deltas: Vec<Delta>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct Delta {
    #[prost(message, repeated, tag = "2")]
    pub(crate) ops: Vec<Op>,
    #[prost(message, optional, tag = "4")]
    pub(crate) info: Option<ChangeInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ChangeInfo {
    #[prost(message, optional, tag = "10")]
    pub(crate) source: Option<SourceInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct SourceInfo {
    #[prost(int32, optional, tag = "1")]
    pub(crate) client: Option<i32>,
}
