use std::fmt;

use crate::content::non_empty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum CardType {
    Unspecified = 0,
    Other = 1,
    Visa = 2,
    Mastercard = 3,
    AmericanExpress = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Expiration {
    pub year: i16,
    pub month: u8,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemCreditCard {
    #[prost(string, tag = "1")]
    pub cardholder_name: String,
    #[prost(enumeration = "CardType", tag = "2")]
    pub card_type: i32,
    #[prost(string, tag = "3")]
    pub number: String,
    #[prost(string, tag = "4")]
    pub verification_number: String,
    #[prost(string, tag = "5")]
    pub expiration_date: String,
    #[prost(string, tag = "6")]
    pub pin: String,
}

impl ItemCreditCard {
    pub fn cardholder(&self) -> Option<&str> {
        non_empty(&self.cardholder_name)
    }

    pub fn expiration(&self) -> Option<Expiration> {
        let (year, month) = self.expiration_date.split_once('-')?;
        if month.len() != 2 {
            return None;
        }
        let expiration = Expiration {
            year: year.parse().ok()?,
            month: month.parse().ok()?,
        };
        (1..=12).contains(&expiration.month).then_some(expiration)
    }
}

impl fmt::Display for Expiration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:04}-{:02}", self.year, self.month)
    }
}
