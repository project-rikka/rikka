use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Product {
    #[default]
    Mail,
    Drive,
    Pass,
    Codes,
    Account,
}

impl Product {
    pub(crate) const ALL: [Product; 5] = [
        Product::Mail,
        Product::Drive,
        Product::Pass,
        Product::Codes,
        Product::Account,
    ];

    pub(crate) fn at(self) -> usize {
        Product::ALL
            .iter()
            .position(|product| *product == self)
            .unwrap_or_default()
    }

    pub(crate) fn title(self) -> Label {
        match self {
            Product::Mail => tr!("proton.product.mail"),
            Product::Drive => tr!("proton.product.drive"),
            Product::Pass => tr!("proton.product.pass"),
            Product::Codes => tr!("proton.product.authenticator"),
            Product::Account => tr!("proton.product.account"),
        }
    }

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Product::Mail => MaterialSymbol::MAIL,
            Product::Drive => MaterialSymbol::CLOUD,
            Product::Pass => MaterialSymbol::SHIELD_LOCK,
            Product::Codes => MaterialSymbol::PASSWORD,
            Product::Account => MaterialSymbol::PERSON,
        }
    }
}
