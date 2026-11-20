use crate::tables::enums::GeneralCategory;

impl GeneralCategory {
    pub const fn is_letter(self) -> bool {
        matches!(
            self,
            Self::LowercaseLetter
                | Self::ModifierLetter
                | Self::OtherLetter
                | Self::TitlecaseLetter
                | Self::UppercaseLetter
        )
    }

    pub const fn is_mark(self) -> bool {
        matches!(
            self,
            Self::SpacingMark | Self::EnclosingMark | Self::NonspacingMark
        )
    }

    pub const fn is_number(self) -> bool {
        matches!(
            self,
            Self::DecimalNumber | Self::LetterNumber | Self::OtherNumber
        )
    }

    pub const fn is_punctuation(self) -> bool {
        matches!(
            self,
            Self::ConnectorPunctuation
                | Self::DashPunctuation
                | Self::ClosePunctuation
                | Self::FinalPunctuation
                | Self::InitialPunctuation
                | Self::OtherPunctuation
                | Self::OpenPunctuation
        )
    }

    pub const fn is_symbol(self) -> bool {
        matches!(
            self,
            Self::CurrencySymbol | Self::ModifierSymbol | Self::MathSymbol | Self::OtherSymbol
        )
    }

    pub const fn is_separator(self) -> bool {
        matches!(
            self,
            Self::LineSeparator | Self::ParagraphSeparator | Self::SpaceSeparator
        )
    }

    pub const fn is_other(self) -> bool {
        matches!(
            self,
            Self::Control | Self::Format | Self::Unassigned | Self::PrivateUse | Self::Surrogate
        )
    }
}
