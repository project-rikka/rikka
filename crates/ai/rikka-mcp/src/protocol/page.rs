use serde::de::DeserializeOwned;
use serde_json::Value;

const NEXT_CURSOR: &str = "nextCursor";

pub(crate) trait Listable: DeserializeOwned {
    const METHOD: &'static str;
    const ITEMS: &'static str;
}

#[derive(Debug)]
pub(crate) struct Page<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_cursor: Option<String>,
}

impl<T: Listable> Page<T> {
    pub(crate) fn from_result(mut result: Value) -> serde_json::Result<Page<T>> {
        let items = match result.get_mut(T::ITEMS).map(Value::take) {
            Some(Value::Null) | None => Vec::new(),
            Some(items) => serde_json::from_value(items)?,
        };
        let next_cursor = result
            .get(NEXT_CURSOR)
            .and_then(Value::as_str)
            .filter(|cursor| !cursor.is_empty())
            .map(str::to_owned);
        Ok(Page { items, next_cursor })
    }
}
