use super::meta::{AvatarPermission, CommercialUsage, CreditNotation, Meta, Modification};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LegacyUser {
    #[default]
    OnlyAuthor,
    ExplicitlyLicensedPerson,
    Everyone,
}

named!(LegacyUser {
    OnlyAuthor => "OnlyAuthor",
    ExplicitlyLicensedPerson => "ExplicitlyLicensedPerson",
    Everyone => "Everyone",
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LicenseName {
    #[default]
    RedistributionProhibited,
    Cc0,
    CcBy,
    CcByNc,
    CcBySa,
    CcByNcSa,
    CcByNd,
    CcByNcNd,
    Other,
}

named!(LicenseName {
    RedistributionProhibited => "Redistribution_Prohibited",
    Cc0 => "CC0",
    CcBy => "CC_BY",
    CcByNc => "CC_BY_NC",
    CcBySa => "CC_BY_SA",
    CcByNcSa => "CC_BY_NC_SA",
    CcByNd => "CC_BY_ND",
    CcByNcNd => "CC_BY_NC_ND",
    Other => "Other",
});

impl LicenseName {
    #[must_use]
    pub fn is_creative_commons(self) -> bool {
        !matches!(self, Self::RedistributionProhibited | Self::Other)
    }

    #[must_use]
    pub fn is_non_commercial(self) -> bool {
        matches!(self, Self::CcByNc | Self::CcByNcSa | Self::CcByNcNd)
    }

    #[must_use]
    pub fn is_no_derivatives(self) -> bool {
        matches!(self, Self::CcByNd | Self::CcByNcNd)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LegacyLicense {
    pub allowed_user: LegacyUser,
    pub violent_usage: bool,
    pub sexual_usage: bool,
    pub commercial_usage: bool,
    pub other_permission_url: Option<String>,
    pub license: LicenseName,
    pub other_license_url: Option<String>,
}

impl LegacyLicense {
    #[must_use]
    pub fn from_meta(meta: &Meta) -> Self {
        let commercial = meta.commercial_usage != CommercialUsage::PersonalNonProfit;
        let derivatives = meta.modification != Modification::Prohibited;
        let license = match (meta.allow_redistribution, commercial, derivatives) {
            (false, _, _) => LicenseName::RedistributionProhibited,
            (true, true, true) if meta.credit_notation == CreditNotation::Unnecessary => {
                LicenseName::Cc0
            }
            (true, true, true) => LicenseName::CcBy,
            (true, false, true) => LicenseName::CcByNc,
            (true, true, false) => LicenseName::CcByNd,
            (true, false, false) => LicenseName::CcByNcNd,
        };
        Self {
            allowed_user: match meta.avatar_permission {
                AvatarPermission::OnlyAuthor => LegacyUser::OnlyAuthor,
                AvatarPermission::OnlySeparatelyLicensedPerson => {
                    LegacyUser::ExplicitlyLicensedPerson
                }
                AvatarPermission::Everyone => LegacyUser::Everyone,
            },
            violent_usage: meta.allow_excessively_violent_usage,
            sexual_usage: meta.allow_excessively_sexual_usage,
            commercial_usage: commercial,
            other_permission_url: None,
            license,
            other_license_url: meta.other_license_url.clone(),
        }
    }

    pub fn apply_to(&self, meta: &mut Meta) {
        let license = self.license;
        meta.avatar_permission = match self.allowed_user {
            LegacyUser::OnlyAuthor => AvatarPermission::OnlyAuthor,
            LegacyUser::ExplicitlyLicensedPerson => AvatarPermission::OnlySeparatelyLicensedPerson,
            LegacyUser::Everyone => AvatarPermission::Everyone,
        };
        meta.commercial_usage = match self.commercial_usage && !license.is_non_commercial() {
            true => CommercialUsage::PersonalProfit,
            false => CommercialUsage::PersonalNonProfit,
        };
        meta.credit_notation = match license {
            LicenseName::Cc0 => CreditNotation::Unnecessary,
            _ => CreditNotation::Required,
        };
        meta.modification = match license.is_creative_commons() && !license.is_no_derivatives() {
            true => Modification::AllowModificationRedistribution,
            false => Modification::Prohibited,
        };
        meta.allow_redistribution = license.is_creative_commons();
        meta.allow_excessively_violent_usage = self.violent_usage;
        meta.allow_excessively_sexual_usage = self.sexual_usage;
        meta.allow_political_or_religious_usage = false;
        meta.allow_antisocial_or_hate_usage = false;
        meta.other_license_url = self
            .other_license_url
            .clone()
            .or_else(|| self.other_permission_url.clone());
    }
}
