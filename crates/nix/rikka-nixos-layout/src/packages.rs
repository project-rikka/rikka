use rikka_nix_edit::{AttrPath, Expr, FromNix};

const SCOPE: &str = "pkgs";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Packages(Vec<AttrPath>);

impl Packages {
    #[must_use]
    pub fn new<S: AsRef<str>>(attributes: impl IntoIterator<Item = S>) -> Self {
        Self(
            attributes
                .into_iter()
                .map(|attribute| {
                    AttrPath::new(attribute.as_ref().split('.'))
                        .expect("split yields at least one segment")
                })
                .collect(),
        )
    }

    pub fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.0.iter().map(|path| path.segments().join("."))
    }
}

fn scope() -> AttrPath {
    AttrPath::new([SCOPE]).expect("scope is a single identifier")
}

fn unscoped(item: Expr) -> Option<AttrPath> {
    let path = item.as_reference()?;
    Some(path.strip_prefix(&scope()).unwrap_or_else(|| path.clone()))
}

impl FromNix for Packages {
    const EXPECTED: &'static str = "a list of packages";

    fn from_nix(expr: Expr) -> Option<Self> {
        let items = match expr {
            Expr::With { scope: named, body } if named.as_reference() == Some(&scope()) => {
                match *body {
                    Expr::List(items) => items,
                    _ => return None,
                }
            }
            Expr::List(items) => items,
            _ => return None,
        };
        items
            .into_iter()
            .map(unscoped)
            .collect::<Option<_>>()
            .map(Self)
    }
}

impl From<Packages> for Expr {
    fn from(packages: Packages) -> Self {
        let items = packages
            .0
            .into_iter()
            .map(|path| match path.is_referable() {
                true => Self::Reference(path),
                false => Self::Reference(scope().join(&path)),
            });
        Self::with(Self::Reference(scope()), Self::list(items))
    }
}
