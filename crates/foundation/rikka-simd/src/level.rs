use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::simd::{Avx2, Avx512, Sse2, Sse42};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Isa {
    Sse2,
    Sse42,
    Avx2,
    Avx512,
}

impl Isa {
    pub const ALL: [Isa; 4] = [Isa::Sse2, Isa::Sse42, Isa::Avx2, Isa::Avx512];

    pub const fn name(self) -> &'static str {
        match self {
            Isa::Sse2 => "sse2",
            Isa::Sse42 => "sse4.2",
            Isa::Avx2 => "avx2",
            Isa::Avx512 => "avx512",
        }
    }

    pub fn is_supported(self) -> bool {
        self <= Level::detect().isa()
    }

    fn probe(self) -> bool {
        match self {
            Isa::Sse2 => true,
            Isa::Sse42 => {
                std::is_x86_feature_detected!("sse3")
                    && std::is_x86_feature_detected!("ssse3")
                    && std::is_x86_feature_detected!("sse4.1")
                    && std::is_x86_feature_detected!("sse4.2")
                    && std::is_x86_feature_detected!("popcnt")
            }
            Isa::Avx2 => {
                std::is_x86_feature_detected!("avx")
                    && std::is_x86_feature_detected!("avx2")
                    && std::is_x86_feature_detected!("fma")
                    && std::is_x86_feature_detected!("bmi1")
                    && std::is_x86_feature_detected!("bmi2")
                    && std::is_x86_feature_detected!("f16c")
                    && std::is_x86_feature_detected!("lzcnt")
                    && std::is_x86_feature_detected!("movbe")
            }
            Isa::Avx512 => {
                std::is_x86_feature_detected!("avx512f")
                    && std::is_x86_feature_detected!("avx512bw")
                    && std::is_x86_feature_detected!("avx512cd")
                    && std::is_x86_feature_detected!("avx512dq")
                    && std::is_x86_feature_detected!("avx512vl")
            }
        }
    }
}

impl fmt::Display for Isa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(Isa);

const UNPROBED: u8 = u8::MAX;

static DETECTED: AtomicU8 = AtomicU8::new(UNPROBED);

impl Level {
    pub fn detect() -> Self {
        let cached = DETECTED.load(Ordering::Relaxed);
        if let Some(&isa) = Isa::ALL.get(usize::from(cached)) {
            return Self(isa);
        }
        let isa = Isa::ALL
            .into_iter()
            .take_while(|isa| isa.probe())
            .last()
            .unwrap_or(Isa::Sse2);
        DETECTED.store(isa as u8, Ordering::Relaxed);
        Self(isa)
    }

    pub const fn baseline() -> Self {
        Self(Isa::Sse2)
    }
    pub(crate) const fn for_token(isa: Isa) -> Self {
        Self(isa)
    }

    pub fn new(isa: Isa) -> Option<Self> {
        isa.is_supported().then_some(Self(isa))
    }

    pub fn supported() -> impl DoubleEndedIterator<Item = Self> + ExactSizeIterator {
        let best = Self::detect().0 as usize;
        Isa::ALL[..=best].iter().map(|&isa| Self(isa))
    }

    pub const fn isa(self) -> Isa {
        self.0
    }

    pub const fn name(self) -> &'static str {
        self.0.name()
    }

    pub fn with_max_isa(self, isa: Isa) -> Self {
        Self(self.0.min(isa))
    }

    #[doc(hidden)]
    #[inline(always)]
    pub fn token(self) -> Token {
        match self.0 {
            Isa::Sse2 => Token::Sse2(Sse2::new()),
            Isa::Sse42 => Token::Sse42(unsafe { Sse42::new_unchecked() }),
            Isa::Avx2 => Token::Avx2(unsafe { Avx2::new_unchecked() }),
            Isa::Avx512 => Token::Avx512(unsafe { Avx512::new_unchecked() }),
        }
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::detect()
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub enum Token {
    Sse2(Sse2),
    Sse42(Sse42),
    Avx2(Avx2),
    Avx512(Avx512),
}
