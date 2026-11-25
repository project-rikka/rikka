#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    End,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    ByteArray,
    String,
    List,
    Compound,
    IntArray,
    LongArray,
}

impl Kind {
    pub(crate) fn from_id(id: u8) -> Option<Kind> {
        Some(match id {
            0 => Kind::End,
            1 => Kind::Byte,
            2 => Kind::Short,
            3 => Kind::Int,
            4 => Kind::Long,
            5 => Kind::Float,
            6 => Kind::Double,
            7 => Kind::ByteArray,
            8 => Kind::String,
            9 => Kind::List,
            10 => Kind::Compound,
            11 => Kind::IntArray,
            12 => Kind::LongArray,
            _ => return None,
        })
    }

    pub(crate) const fn id(self) -> u8 {
        match self {
            Kind::End => 0,
            Kind::Byte => 1,
            Kind::Short => 2,
            Kind::Int => 3,
            Kind::Long => 4,
            Kind::Float => 5,
            Kind::Double => 6,
            Kind::ByteArray => 7,
            Kind::String => 8,
            Kind::List => 9,
            Kind::Compound => 10,
            Kind::IntArray => 11,
            Kind::LongArray => 12,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct List {
    kind: Kind,
    items: Vec<Tag>,
}

impl List {
    #[must_use]
    pub fn empty() -> List {
        List {
            kind: Kind::End,
            items: Vec::new(),
        }
    }

    pub(crate) fn from_parts(kind: Kind, items: Vec<Tag>) -> List {
        List { kind, items }
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn items(&self) -> &[Tag] {
        &self.items
    }

    pub fn push(&mut self, tag: Tag) -> crate::Result<()> {
        if self.items.is_empty() {
            self.kind = tag.kind();
        } else if tag.kind() != self.kind {
            return Err(crate::Error::Mixed {
                expected: self.kind,
                found: tag.kind(),
            });
        }
        self.items.push(tag);
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Tag {
        let removed = self.items.remove(index);
        if self.items.is_empty() {
            self.kind = Kind::End;
        }
        removed
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound(Vec<(String, Tag)>);

impl Compound {
    #[must_use]
    pub fn new() -> Compound {
        Compound::default()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Tag> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, tag)| tag)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Tag> {
        self.0
            .iter_mut()
            .find(|(key, _)| key == name)
            .map(|(_, tag)| tag)
    }

    pub fn insert(&mut self, name: impl Into<String>, tag: Tag) {
        let name = name.into();
        match self.get_mut(&name) {
            Some(existing) => *existing = tag,
            None => self.0.push((name, tag)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Tag)> {
        self.0.iter().map(|(name, tag)| (name.as_str(), tag))
    }

    #[must_use]
    pub fn string(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            Tag::String(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn compound(&self, name: &str) -> Option<&Compound> {
        match self.get(name)? {
            Tag::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    #[must_use]
    pub fn integer(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            Tag::Byte(value) => Some(i64::from(*value)),
            Tag::Short(value) => Some(i64::from(*value)),
            Tag::Int(value) => Some(i64::from(*value)),
            Tag::Long(value) => Some(*value),
            _ => None,
        }
    }

    #[must_use]
    pub fn list(&self, name: &str) -> Option<&List> {
        match self.get(name)? {
            Tag::List(list) => Some(list),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(List),
    Compound(Compound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Tag::Byte(_) => Kind::Byte,
            Tag::Short(_) => Kind::Short,
            Tag::Int(_) => Kind::Int,
            Tag::Long(_) => Kind::Long,
            Tag::Float(_) => Kind::Float,
            Tag::Double(_) => Kind::Double,
            Tag::ByteArray(_) => Kind::ByteArray,
            Tag::String(_) => Kind::String,
            Tag::List(_) => Kind::List,
            Tag::Compound(_) => Kind::Compound,
            Tag::IntArray(_) => Kind::IntArray,
            Tag::LongArray(_) => Kind::LongArray,
        }
    }
}
