#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId {
    pub service: String,
    pub path: String,
}

const DEFAULT_PATH: &str = "/StatusNotifierItem";

impl ItemId {
    #[must_use]
    pub fn new(service: impl Into<String>, path: impl Into<String>) -> ItemId {
        ItemId {
            service: service.into(),
            path: path.into(),
        }
    }

    #[must_use]
    pub(crate) fn registered(argument: &str, sender: &str) -> ItemId {
        let argument = argument.trim();
        if let Some(path) = argument.strip_prefix('/') {
            return ItemId::new(sender, format!("/{path}"));
        }
        match argument.split_once('/') {
            Some((service, path)) => ItemId::new(service, format!("/{path}")),
            None if argument.is_empty() => ItemId::new(sender, DEFAULT_PATH),
            None => ItemId::new(argument, DEFAULT_PATH),
        }
    }

    #[must_use]
    pub(crate) fn listed(entry: &str) -> Option<ItemId> {
        let (service, path) = entry.trim().split_once('/')?;
        (!service.is_empty()).then(|| ItemId::new(service, format!("/{path}")))
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.service, self.path)
    }
}
