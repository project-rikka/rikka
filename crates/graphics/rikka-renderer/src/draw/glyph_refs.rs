use crate::text::atlas::{AtlasKey, GlyphRef};
use crate::text::routing::GlyphRoute;

#[derive(Clone, Debug, Default)]
pub(crate) struct GlyphRefs {
    routes: Vec<GlyphRoute>,
    refs: Vec<Option<GlyphRef>>,
}

impl GlyphRefs {
    pub(crate) fn matches(&self, routes: &[GlyphRoute]) -> bool {
        !routes.is_empty() && self.routes == routes
    }

    pub(crate) fn refs(&self) -> &[Option<GlyphRef>] {
        &self.refs
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = AtlasKey> + '_ {
        self.refs.iter().flatten().filter_map(GlyphRef::placed_key)
    }

    pub(crate) fn retain(
        &mut self,
        routes: &[GlyphRoute],
        refs: &[Option<GlyphRef>],
        dropped: &mut Vec<AtlasKey>,
    ) {
        dropped.extend(self.keys());
        self.routes.clear();
        self.routes.extend_from_slice(routes);
        self.refs.clear();
        self.refs.extend_from_slice(refs);
    }

    pub(crate) fn release(&mut self, dropped: &mut Vec<AtlasKey>) {
        self.retain(&[], &[], dropped);
    }
}
