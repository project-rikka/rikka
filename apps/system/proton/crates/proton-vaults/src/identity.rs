use rikka_proton_pass::{ExtraField, ItemIdentity};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentitySection {
    Personal,
    Address,
    Contact,
    Work,
}

impl IdentitySection {
    pub const ALL: [IdentitySection; 4] = [
        IdentitySection::Personal,
        IdentitySection::Address,
        IdentitySection::Contact,
        IdentitySection::Work,
    ];

    pub fn fields(self) -> impl Iterator<Item = (usize, IdentityField)> {
        IdentityField::ALL
            .into_iter()
            .enumerate()
            .filter(move |(_, field)| field.group() == self)
    }

    #[must_use]
    pub fn extras(self, identity: &ItemIdentity) -> &[ExtraField] {
        match self {
            IdentitySection::Personal => &identity.extra_personal_details,
            IdentitySection::Address => &identity.extra_address_details,
            IdentitySection::Contact => &identity.extra_contact_details,
            IdentitySection::Work => &identity.extra_work_details,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentityField {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    Birthdate,
    Gender,
    Organization,
    Street,
    Floor,
    City,
    PostalCode,
    Province,
    County,
    Country,
    SocialSecurity,
    Passport,
    Licence,
    Website,
    SecondPhone,
    LinkedIn,
    XHandle,
    Instagram,
    Facebook,
    Reddit,
    Yahoo,
    Company,
    Job,
    WorkPhone,
    WorkEmail,
    PersonalWebsite,
}

impl IdentityField {
    pub const ALL: [IdentityField; 32] = [
        IdentityField::FullName,
        IdentityField::FirstName,
        IdentityField::MiddleName,
        IdentityField::LastName,
        IdentityField::Email,
        IdentityField::Phone,
        IdentityField::Birthdate,
        IdentityField::Gender,
        IdentityField::Organization,
        IdentityField::Street,
        IdentityField::Floor,
        IdentityField::City,
        IdentityField::PostalCode,
        IdentityField::Province,
        IdentityField::County,
        IdentityField::Country,
        IdentityField::SocialSecurity,
        IdentityField::Passport,
        IdentityField::Licence,
        IdentityField::Website,
        IdentityField::SecondPhone,
        IdentityField::LinkedIn,
        IdentityField::XHandle,
        IdentityField::Instagram,
        IdentityField::Facebook,
        IdentityField::Reddit,
        IdentityField::Yahoo,
        IdentityField::Company,
        IdentityField::Job,
        IdentityField::WorkPhone,
        IdentityField::WorkEmail,
        IdentityField::PersonalWebsite,
    ];

    #[must_use]
    pub fn group(self) -> IdentitySection {
        match self {
            IdentityField::FullName
            | IdentityField::FirstName
            | IdentityField::MiddleName
            | IdentityField::LastName
            | IdentityField::Email
            | IdentityField::Phone
            | IdentityField::Birthdate
            | IdentityField::Gender => IdentitySection::Personal,
            IdentityField::Organization
            | IdentityField::Street
            | IdentityField::Floor
            | IdentityField::City
            | IdentityField::PostalCode
            | IdentityField::Province
            | IdentityField::County
            | IdentityField::Country => IdentitySection::Address,
            IdentityField::SocialSecurity
            | IdentityField::Passport
            | IdentityField::Licence
            | IdentityField::Website
            | IdentityField::SecondPhone
            | IdentityField::LinkedIn
            | IdentityField::XHandle
            | IdentityField::Instagram
            | IdentityField::Facebook
            | IdentityField::Reddit
            | IdentityField::Yahoo => IdentitySection::Contact,
            IdentityField::Company
            | IdentityField::Job
            | IdentityField::WorkPhone
            | IdentityField::WorkEmail
            | IdentityField::PersonalWebsite => IdentitySection::Work,
        }
    }

    #[must_use]
    pub fn is_hidden(self) -> bool {
        self == IdentityField::SocialSecurity
    }

    #[must_use]
    pub fn read(self, identity: &ItemIdentity) -> &str {
        match self {
            IdentityField::FullName => &identity.full_name,
            IdentityField::FirstName => &identity.first_name,
            IdentityField::MiddleName => &identity.middle_name,
            IdentityField::LastName => &identity.last_name,
            IdentityField::Email => &identity.email,
            IdentityField::Phone => &identity.phone_number,
            IdentityField::Birthdate => &identity.birthdate,
            IdentityField::Gender => &identity.gender,
            IdentityField::Organization => &identity.organization,
            IdentityField::Street => &identity.street_address,
            IdentityField::Floor => &identity.floor,
            IdentityField::City => &identity.city,
            IdentityField::PostalCode => &identity.zip_or_postal_code,
            IdentityField::Province => &identity.state_or_province,
            IdentityField::County => &identity.county,
            IdentityField::Country => &identity.country_or_region,
            IdentityField::SocialSecurity => &identity.social_security_number,
            IdentityField::Passport => &identity.passport_number,
            IdentityField::Licence => &identity.license_number,
            IdentityField::Website => &identity.website,
            IdentityField::SecondPhone => &identity.second_phone_number,
            IdentityField::LinkedIn => &identity.linkedin,
            IdentityField::XHandle => &identity.x_handle,
            IdentityField::Instagram => &identity.instagram,
            IdentityField::Facebook => &identity.facebook,
            IdentityField::Reddit => &identity.reddit,
            IdentityField::Yahoo => &identity.yahoo,
            IdentityField::Company => &identity.company,
            IdentityField::Job => &identity.job_title,
            IdentityField::WorkPhone => &identity.work_phone_number,
            IdentityField::WorkEmail => &identity.work_email,
            IdentityField::PersonalWebsite => &identity.personal_website,
        }
    }

    pub fn write(self, identity: &mut ItemIdentity, value: String) {
        let slot = match self {
            IdentityField::FullName => &mut identity.full_name,
            IdentityField::FirstName => &mut identity.first_name,
            IdentityField::MiddleName => &mut identity.middle_name,
            IdentityField::LastName => &mut identity.last_name,
            IdentityField::Email => &mut identity.email,
            IdentityField::Phone => &mut identity.phone_number,
            IdentityField::Birthdate => &mut identity.birthdate,
            IdentityField::Gender => &mut identity.gender,
            IdentityField::Organization => &mut identity.organization,
            IdentityField::Street => &mut identity.street_address,
            IdentityField::Floor => &mut identity.floor,
            IdentityField::City => &mut identity.city,
            IdentityField::PostalCode => &mut identity.zip_or_postal_code,
            IdentityField::Province => &mut identity.state_or_province,
            IdentityField::County => &mut identity.county,
            IdentityField::Country => &mut identity.country_or_region,
            IdentityField::SocialSecurity => &mut identity.social_security_number,
            IdentityField::Passport => &mut identity.passport_number,
            IdentityField::Licence => &mut identity.license_number,
            IdentityField::Website => &mut identity.website,
            IdentityField::SecondPhone => &mut identity.second_phone_number,
            IdentityField::LinkedIn => &mut identity.linkedin,
            IdentityField::XHandle => &mut identity.x_handle,
            IdentityField::Instagram => &mut identity.instagram,
            IdentityField::Facebook => &mut identity.facebook,
            IdentityField::Reddit => &mut identity.reddit,
            IdentityField::Yahoo => &mut identity.yahoo,
            IdentityField::Company => &mut identity.company,
            IdentityField::Job => &mut identity.job_title,
            IdentityField::WorkPhone => &mut identity.work_phone_number,
            IdentityField::WorkEmail => &mut identity.work_email,
            IdentityField::PersonalWebsite => &mut identity.personal_website,
        };

        *slot = value;
    }
}
