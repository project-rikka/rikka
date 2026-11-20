use crate::domain::{Domain, DomainSizes};
use crate::values::{AttributeValue, DataType, Values};

#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub domain: Domain,
    pub values: Values,
}

impl Attribute {
    pub fn new(name: impl Into<String>, domain: Domain, values: Values) -> Attribute {
        Attribute {
            name: name.into(),
            domain,
            values,
        }
    }

    #[must_use]
    pub fn data_type(&self) -> DataType {
        self.values.data_type()
    }

    #[must_use]
    pub fn typed<T: AttributeValue>(&self) -> Option<&[T]> {
        self.values.typed()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attributes {
    list: Vec<Attribute>,
}

impl Attributes {
    #[must_use]
    pub fn new() -> Attributes {
        Attributes::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Attribute> {
        self.list.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Attribute> {
        self.list.iter_mut()
    }

    pub fn in_domain(&self, domain: Domain) -> impl Iterator<Item = &Attribute> {
        self.list
            .iter()
            .filter(move |attribute| attribute.domain == domain)
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.position(name).is_some()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Attribute> {
        self.list.iter().find(|attribute| attribute.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Attribute> {
        self.list
            .iter_mut()
            .find(|attribute| attribute.name == name)
    }

    #[must_use]
    pub fn values<T: AttributeValue>(&self, name: &str, domain: Domain) -> Option<&[T]> {
        self.get(name)
            .filter(|attribute| attribute.domain == domain)
            .and_then(|attribute| attribute.values.typed())
    }

    pub fn values_mut<T: AttributeValue>(
        &mut self,
        name: &str,
        domain: Domain,
    ) -> Option<&mut [T]> {
        self.get_mut(name)
            .filter(|attribute| attribute.domain == domain)
            .and_then(|attribute| attribute.values.typed_mut())
    }

    pub fn insert(&mut self, attribute: Attribute) -> Option<Attribute> {
        match self.position(&attribute.name) {
            Some(index) => Some(std::mem::replace(&mut self.list[index], attribute)),
            None => {
                self.list.push(attribute);
                None
            }
        }
    }

    pub fn remove(&mut self, name: &str) -> Option<Attribute> {
        let index = self.position(name)?;
        Some(self.list.remove(index))
    }

    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        if from == to {
            return self.contains(from);
        }
        if self.contains(to) {
            return false;
        }
        match self.get_mut(from) {
            Some(attribute) => {
                to.clone_into(&mut attribute.name);
                true
            }
            None => false,
        }
    }

    pub fn ensure<T: AttributeValue>(
        &mut self,
        name: &str,
        domain: Domain,
        len: usize,
    ) -> &mut [T] {
        let index = match self.position(name) {
            Some(index) => {
                let attribute = &mut self.list[index];
                if attribute.domain != domain || attribute.values.len() != len {
                    attribute.domain = domain;
                    attribute.values = Values::new(T::DATA_TYPE, len);
                } else if attribute.data_type() != T::DATA_TYPE {
                    attribute.values = attribute.values.convert(T::DATA_TYPE);
                }
                index
            }
            None => {
                self.list
                    .push(Attribute::new(name, domain, Values::new(T::DATA_TYPE, len)));
                self.list.len() - 1
            }
        };
        self.list[index]
            .values
            .typed_mut()
            .expect("the attribute was given the requested type")
    }

    pub fn retain(&mut self, keep: impl FnMut(&Attribute) -> bool) {
        self.list.retain(keep);
    }

    pub fn resize(&mut self, domain: Domain, len: usize) {
        self.each_in(domain, |values| values.resize(len));
    }

    pub fn gather(&mut self, domain: Domain, indices: &[u32]) {
        self.each_in(domain, |values| values.gather(indices));
    }

    pub fn copy(&mut self, domain: Domain, from: usize, to: usize) {
        self.each_in(domain, |values| values.copy(from, to));
    }

    pub fn interpolate(&mut self, domain: Domain, sources: &[(u32, f32)], to: usize) {
        self.each_in(domain, |values| values.interpolate(sources, to));
    }

    pub fn push_copy(&mut self, domain: Domain, from: usize) {
        self.each_in(domain, |values| values.push_copy(from));
    }

    pub fn push_interpolated(&mut self, domain: Domain, sources: &[(u32, f32)]) {
        self.each_in(domain, |values| values.push_interpolated(sources));
    }

    pub fn extend_from(
        &mut self,
        other: &Attributes,
        sizes: DomainSizes,
        other_sizes: DomainSizes,
    ) {
        for attribute in &mut self.list {
            let domain = attribute.domain;
            match other
                .get(&attribute.name)
                .filter(|added| added.domain == domain)
            {
                Some(added) => attribute.values.extend(&added.values),
                None => attribute
                    .values
                    .resize(sizes.get(domain) + other_sizes.get(domain)),
            }
        }
        for added in &other.list {
            if self.contains(&added.name) {
                continue;
            }
            let mut values = Values::new(added.data_type(), sizes.get(added.domain));
            values.extend(&added.values);
            self.list
                .push(Attribute::new(added.name.clone(), added.domain, values));
        }
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.list
            .iter()
            .position(|attribute| attribute.name == name)
    }

    fn each_in(&mut self, domain: Domain, mut change: impl FnMut(&mut Values)) {
        for attribute in &mut self.list {
            if attribute.domain == domain {
                change(&mut attribute.values);
            }
        }
    }
}
