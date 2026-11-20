use xkbcommon::xkb;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
    pub caps_lock: bool,
    pub num_lock: bool,
    pub level3: bool,
    pub level5: bool,
}

impl Modifiers {
    pub(crate) fn from_effective(effective: xkb::ModMask, masks: &ModMasks) -> Self {
        let active = |mask: xkb::ModMask| effective & mask != 0;
        Self {
            shift: active(masks.shift),
            ctrl: active(masks.ctrl),
            alt: active(masks.alt),
            logo: active(masks.logo),
            caps_lock: active(masks.caps_lock),
            num_lock: active(masks.num_lock),
            level3: active(masks.level3),
            level5: active(masks.level5),
        }
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers {
            shift: self.shift || other.shift,
            ctrl: self.ctrl || other.ctrl,
            alt: self.alt || other.alt,
            logo: self.logo || other.logo,
            caps_lock: self.caps_lock || other.caps_lock,
            num_lock: self.num_lock || other.num_lock,
            level3: self.level3 || other.level3,
            level5: self.level5 || other.level5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ModMasks {
    shift: xkb::ModMask,
    ctrl: xkb::ModMask,
    alt: xkb::ModMask,
    logo: xkb::ModMask,
    caps_lock: xkb::ModMask,
    num_lock: xkb::ModMask,
    level3: xkb::ModMask,
    level5: xkb::ModMask,
}

impl ModMasks {
    pub(crate) fn from_keymap(keymap: &xkb::Keymap) -> Self {
        let mask = |name: &str| {
            let index = keymap.mod_get_index(name);
            1u32.checked_shl(index).unwrap_or(0)
        };
        Self {
            shift: mask(xkb::MOD_NAME_SHIFT),
            ctrl: mask(xkb::MOD_NAME_CTRL),
            alt: mask(xkb::MOD_NAME_ALT),
            logo: mask(xkb::MOD_NAME_LOGO),
            caps_lock: mask(xkb::MOD_NAME_CAPS),
            num_lock: mask(xkb::MOD_NAME_NUM),
            level3: mask(xkb::MOD_NAME_ISO_LEVEL3_SHIFT),
            level5: mask(xkb::MOD_NAME_MOD3),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SerializedModifiers {
    pub depressed: u32,
    pub latched: u32,
    pub locked: u32,
    pub group: u32,
}

impl SerializedModifiers {
    pub(crate) fn from_state(state: &xkb::State) -> Self {
        Self {
            depressed: state.serialize_mods(xkb::STATE_MODS_DEPRESSED),
            latched: state.serialize_mods(xkb::STATE_MODS_LATCHED),
            locked: state.serialize_mods(xkb::STATE_MODS_LOCKED),
            group: state.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE),
        }
    }

    pub(crate) fn apply(self, state: &mut xkb::State) -> xkb::StateComponent {
        state.update_mask(self.depressed, self.latched, self.locked, 0, 0, self.group)
    }
}
