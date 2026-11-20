use crate::content::extra::ExtraField;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct CustomSection {
    #[prost(string, tag = "1")]
    pub section_name: String,
    #[prost(message, repeated, tag = "2")]
    pub section_fields: Vec<ExtraField>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemCustom {
    #[prost(message, repeated, tag = "1")]
    pub sections: Vec<CustomSection>,
}

impl CustomSection {
    pub fn name(&self) -> &str {
        &self.section_name
    }

    pub fn field(&self, name: &str) -> Option<&ExtraField> {
        self.section_fields
            .iter()
            .find(|field| field.field_name == name)
    }
}
