#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Device {
    path: String,
    name: String,
    vendor: u16,
    product: u16,
}

impl Device {
    #[must_use]
    pub fn new(path: impl Into<String>) -> Device {
        Device {
            path: path.into(),
            name: String::new(),
            vendor: 0,
            product: 0,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Device {
        self.name = name.into();
        self
    }

    #[must_use]
    pub fn with_ids(mut self, vendor: u16, product: u16) -> Device {
        self.vendor = vendor;
        self.product = product;
        self
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn vendor(&self) -> u16 {
        self.vendor
    }

    #[must_use]
    pub fn product(&self) -> u16 {
        self.product
    }
}
