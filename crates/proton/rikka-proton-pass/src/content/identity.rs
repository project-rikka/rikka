use crate::content::custom::CustomSection;
use crate::content::extra::ExtraField;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemIdentity {
    #[prost(string, tag = "1")]
    pub full_name: String,
    #[prost(string, tag = "2")]
    pub email: String,
    #[prost(string, tag = "3")]
    pub phone_number: String,
    #[prost(string, tag = "4")]
    pub first_name: String,
    #[prost(string, tag = "5")]
    pub middle_name: String,
    #[prost(string, tag = "6")]
    pub last_name: String,
    #[prost(string, tag = "7")]
    pub birthdate: String,
    #[prost(string, tag = "8")]
    pub gender: String,
    #[prost(message, repeated, tag = "9")]
    pub extra_personal_details: Vec<ExtraField>,
    #[prost(string, tag = "10")]
    pub organization: String,
    #[prost(string, tag = "11")]
    pub street_address: String,
    #[prost(string, tag = "12")]
    pub zip_or_postal_code: String,
    #[prost(string, tag = "13")]
    pub city: String,
    #[prost(string, tag = "14")]
    pub state_or_province: String,
    #[prost(string, tag = "15")]
    pub country_or_region: String,
    #[prost(string, tag = "16")]
    pub floor: String,
    #[prost(string, tag = "17")]
    pub county: String,
    #[prost(message, repeated, tag = "18")]
    pub extra_address_details: Vec<ExtraField>,
    #[prost(string, tag = "19")]
    pub social_security_number: String,
    #[prost(string, tag = "20")]
    pub passport_number: String,
    #[prost(string, tag = "21")]
    pub license_number: String,
    #[prost(string, tag = "22")]
    pub website: String,
    #[prost(string, tag = "23")]
    pub x_handle: String,
    #[prost(string, tag = "24")]
    pub second_phone_number: String,
    #[prost(string, tag = "25")]
    pub linkedin: String,
    #[prost(string, tag = "26")]
    pub reddit: String,
    #[prost(string, tag = "27")]
    pub facebook: String,
    #[prost(string, tag = "28")]
    pub yahoo: String,
    #[prost(string, tag = "29")]
    pub instagram: String,
    #[prost(message, repeated, tag = "30")]
    pub extra_contact_details: Vec<ExtraField>,
    #[prost(string, tag = "31")]
    pub company: String,
    #[prost(string, tag = "32")]
    pub job_title: String,
    #[prost(string, tag = "33")]
    pub personal_website: String,
    #[prost(string, tag = "34")]
    pub work_phone_number: String,
    #[prost(string, tag = "35")]
    pub work_email: String,
    #[prost(message, repeated, tag = "36")]
    pub extra_work_details: Vec<ExtraField>,
    #[prost(message, repeated, tag = "37")]
    pub extra_sections: Vec<CustomSection>,
}

impl ItemIdentity {
    pub fn details(&self) -> impl Iterator<Item = &ExtraField> {
        self.extra_personal_details
            .iter()
            .chain(&self.extra_address_details)
            .chain(&self.extra_contact_details)
            .chain(&self.extra_work_details)
    }
}
