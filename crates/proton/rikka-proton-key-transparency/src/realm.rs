use crate::vrf::VrfKey;

const PRODUCTION_DOMAIN: &str = "keytransparency.ch";
const ATLAS_DOMAIN: &str = "dev.proton.wtf";
const SCIENTIST_SUFFIX: &str = ".proton.wtf";

const PRODUCTION_VRF_KEY: VrfKey = VrfKey::new([
    0x90, 0xa7, 0x4b, 0x3d, 0x3a, 0xd9, 0xcb, 0x92, 0xe6, 0x2c, 0x4e, 0x9c, 0x32, 0xac, 0xc3, 0xcc,
    0x3e, 0xff, 0x19, 0x9a, 0x32, 0xa0, 0xab, 0x47, 0xa0, 0x58, 0x72, 0xc2, 0xf1, 0x5a, 0x99, 0xc6,
]);

const DEVELOPMENT_VRF_KEY: VrfKey = VrfKey::new([
    0x2d, 0x76, 0x88, 0xfe, 0xb4, 0x29, 0xf7, 0x14, 0xf1, 0x02, 0xf7, 0x58, 0x41, 0x2c, 0xd4, 0xb8,
    0x13, 0x37, 0xb3, 0x07, 0x12, 0x27, 0x70, 0xf6, 0x20, 0xad, 0x9e, 0x4a, 0xc8, 0x98, 0xa2, 0xeb,
]);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Realm {
    domain: String,
    vrf_key: VrfKey,
}

impl Realm {
    #[must_use]
    pub fn production() -> Realm {
        Realm::new(PRODUCTION_DOMAIN, PRODUCTION_VRF_KEY)
    }

    #[must_use]
    pub fn atlas() -> Realm {
        Realm::new(ATLAS_DOMAIN, DEVELOPMENT_VRF_KEY)
    }

    #[must_use]
    pub fn scientist(name: &str) -> Realm {
        Realm::new(format!("{name}{SCIENTIST_SUFFIX}"), DEVELOPMENT_VRF_KEY)
    }

    #[must_use]
    pub fn new(domain: impl Into<String>, vrf_key: VrfKey) -> Realm {
        Realm {
            domain: domain.into(),
            vrf_key,
        }
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn vrf_key(&self) -> &VrfKey {
        &self.vrf_key
    }
}

impl Default for Realm {
    fn default() -> Realm {
        Realm::production()
    }
}
