use rikka_model_core::ImageId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarPermission {
    #[default]
    OnlyAuthor,
    OnlySeparatelyLicensedPerson,
    Everyone,
}

named!(AvatarPermission {
    OnlyAuthor => "onlyAuthor",
    OnlySeparatelyLicensedPerson => "onlySeparatelyLicensedPerson",
    Everyone => "everyone",
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommercialUsage {
    #[default]
    PersonalNonProfit,
    PersonalProfit,
    Corporation,
}

named!(CommercialUsage {
    PersonalNonProfit => "personalNonProfit",
    PersonalProfit => "personalProfit",
    Corporation => "corporation",
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CreditNotation {
    #[default]
    Required,
    Unnecessary,
}

named!(CreditNotation {
    Required => "required",
    Unnecessary => "unnecessary",
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Modification {
    #[default]
    Prohibited,
    AllowModification,
    AllowModificationRedistribution,
}

named!(Modification {
    Prohibited => "prohibited",
    AllowModification => "allowModification",
    AllowModificationRedistribution => "allowModificationRedistribution",
});

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub name: String,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub copyright: Option<String>,
    pub contact: Option<String>,
    pub references: Vec<String>,
    pub third_party_licenses: Option<String>,
    pub thumbnail: Option<ImageId>,
    pub license_url: String,
    pub other_license_url: Option<String>,
    pub avatar_permission: AvatarPermission,
    pub commercial_usage: CommercialUsage,
    pub credit_notation: CreditNotation,
    pub modification: Modification,
    pub allow_redistribution: bool,
    pub allow_excessively_violent_usage: bool,
    pub allow_excessively_sexual_usage: bool,
    pub allow_political_or_religious_usage: bool,
    pub allow_antisocial_or_hate_usage: bool,
}
