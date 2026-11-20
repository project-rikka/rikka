use crate::attributes::Pasting;
use crate::export::Exporting;
use crate::reformat::Reformatting;
use crate::relink::Relinking;

#[derive(Debug)]
pub(crate) enum Sheet {
    Export(Box<Exporting>),
    Reformat(Reformatting),
    Relink(Relinking),
    PasteAttributes(Box<Pasting>),
}

pub(crate) trait Draft: Sized {
    fn of(sheet: &Sheet) -> Option<&Self>;

    fn of_mut(sheet: &mut Sheet) -> Option<&mut Self>;
}

macro_rules! drafts {
    ($($draft:ty => $variant:ident,)+) => {
        $(
            impl Draft for $draft {
                fn of(sheet: &Sheet) -> Option<&Self> {
                    match sheet {
                        Sheet::$variant(draft) => Some(draft),
                        _ => None,
                    }
                }

                fn of_mut(sheet: &mut Sheet) -> Option<&mut Self> {
                    match sheet {
                        Sheet::$variant(draft) => Some(draft),
                        _ => None,
                    }
                }
            }
        )+
    };
}

drafts! {
    Exporting => Export,
    Reformatting => Reformat,
    Relinking => Relink,
    Pasting => PasteAttributes,
}

pub(crate) fn draft<D: Draft>(sheet: &Option<Sheet>) -> Option<&D> {
    sheet.as_ref().and_then(D::of)
}

pub(crate) fn draft_mut<D: Draft>(sheet: &mut Option<Sheet>) -> Option<&mut D> {
    sheet.as_mut().and_then(D::of_mut)
}
