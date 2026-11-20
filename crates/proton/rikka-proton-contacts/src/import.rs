use crate::error::Result;
use crate::vcard::VCard;

pub fn parse_vcf(text: &str) -> Result<Vec<VCard>> {
    Ok(VCard::parse_all(text)?.iter().map(VCard::to_v4).collect())
}
