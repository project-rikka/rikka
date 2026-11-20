use crate::sfnt::tag::Tag;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TableId {
    Head,
    Hhea,
    Hmtx,
    Vhea,
    Vmtx,
    Maxp,
    Os2,
    Name,
    Post,
    Cmap,
    Loca,
    Glyf,
    Cff,
    Cff2,
    Fvar,
    Avar,
    Gvar,
    Hvar,
    Vvar,
    Mvar,
    Stat,
    Gdef,
    Gsub,
    Gpos,
    Kern,
    Cpal,
    Colr,
    Cblc,
    Cbdt,
    Eblc,
    Ebdt,
    Sbix,
    Svg,
}

impl TableId {
    pub(crate) const COUNT: usize = 33;

    const ALL: [(Tag, TableId); Self::COUNT] = [
        (Tag::new(b"head"), Self::Head),
        (Tag::new(b"hhea"), Self::Hhea),
        (Tag::new(b"hmtx"), Self::Hmtx),
        (Tag::new(b"vhea"), Self::Vhea),
        (Tag::new(b"vmtx"), Self::Vmtx),
        (Tag::new(b"maxp"), Self::Maxp),
        (Tag::new(b"OS/2"), Self::Os2),
        (Tag::new(b"name"), Self::Name),
        (Tag::new(b"post"), Self::Post),
        (Tag::new(b"cmap"), Self::Cmap),
        (Tag::new(b"loca"), Self::Loca),
        (Tag::new(b"glyf"), Self::Glyf),
        (Tag::new(b"CFF "), Self::Cff),
        (Tag::new(b"CFF2"), Self::Cff2),
        (Tag::new(b"fvar"), Self::Fvar),
        (Tag::new(b"avar"), Self::Avar),
        (Tag::new(b"gvar"), Self::Gvar),
        (Tag::new(b"HVAR"), Self::Hvar),
        (Tag::new(b"VVAR"), Self::Vvar),
        (Tag::new(b"MVAR"), Self::Mvar),
        (Tag::new(b"STAT"), Self::Stat),
        (Tag::new(b"GDEF"), Self::Gdef),
        (Tag::new(b"GSUB"), Self::Gsub),
        (Tag::new(b"GPOS"), Self::Gpos),
        (Tag::new(b"kern"), Self::Kern),
        (Tag::new(b"CPAL"), Self::Cpal),
        (Tag::new(b"COLR"), Self::Colr),
        (Tag::new(b"CBLC"), Self::Cblc),
        (Tag::new(b"CBDT"), Self::Cbdt),
        (Tag::new(b"EBLC"), Self::Eblc),
        (Tag::new(b"EBDT"), Self::Ebdt),
        (Tag::new(b"sbix"), Self::Sbix),
        (Tag::new(b"SVG "), Self::Svg),
    ];

    pub(crate) fn from_tag(tag: Tag) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(known, _)| *known == tag)
            .map(|&(_, id)| id)
    }

    pub(crate) fn tag(self) -> Tag {
        Self::ALL[self as usize].0
    }
}
