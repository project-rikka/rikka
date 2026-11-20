#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ItemFlags(i64);

impl ItemFlags {
    const SKIP_HEALTH_CHECK: i64 = 1 << 0;
    const EMAIL_BREACHED: i64 = 1 << 1;
    const ALIAS_DISABLED: i64 = 1 << 2;
    const HAS_ATTACHMENTS: i64 = 1 << 3;
    const HAD_ATTACHMENTS: i64 = 1 << 4;
    const SKIP_WEAK_PASSWORD_CHECK: i64 = 1 << 5;
    const SKIP_COMPROMISED_PASSWORD_CHECK: i64 = 1 << 6;
    const SKIP_REUSED_PASSWORD_CHECK: i64 = 1 << 7;
    const SKIP_TWO_FACTOR_CHECK: i64 = 1 << 8;

    #[must_use]
    pub fn new(bits: i64) -> ItemFlags {
        ItemFlags(bits)
    }

    #[must_use]
    pub fn bits(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn skips_health_check(self) -> bool {
        self.contains_any(ItemFlags::SKIP_HEALTH_CHECK)
    }

    #[must_use]
    pub fn is_email_breached(self) -> bool {
        self.contains_any(ItemFlags::EMAIL_BREACHED)
    }

    #[must_use]
    pub fn is_alias_disabled(self) -> bool {
        self.contains_any(ItemFlags::ALIAS_DISABLED)
    }

    #[must_use]
    pub fn has_attachments(self) -> bool {
        self.contains_any(ItemFlags::HAS_ATTACHMENTS)
    }

    #[must_use]
    pub fn had_attachments(self) -> bool {
        self.contains_any(ItemFlags::HAD_ATTACHMENTS)
    }

    #[must_use]
    pub fn skips_weak_password_check(self) -> bool {
        self.contains_any(ItemFlags::SKIP_HEALTH_CHECK | ItemFlags::SKIP_WEAK_PASSWORD_CHECK)
    }

    #[must_use]
    pub fn skips_compromised_password_check(self) -> bool {
        self.contains_any(ItemFlags::SKIP_HEALTH_CHECK | ItemFlags::SKIP_COMPROMISED_PASSWORD_CHECK)
    }

    #[must_use]
    pub fn skips_reused_password_check(self) -> bool {
        self.contains_any(ItemFlags::SKIP_HEALTH_CHECK | ItemFlags::SKIP_REUSED_PASSWORD_CHECK)
    }

    #[must_use]
    pub fn skips_two_factor_check(self) -> bool {
        self.contains_any(ItemFlags::SKIP_HEALTH_CHECK | ItemFlags::SKIP_TWO_FACTOR_CHECK)
    }

    fn contains_any(self, bits: i64) -> bool {
        self.0 & bits != 0
    }
}
