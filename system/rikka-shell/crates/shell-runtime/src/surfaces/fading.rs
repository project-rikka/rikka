use super::Surfaces;

impl Surfaces {
    pub(crate) fn follow_opacities(&mut self) {
        let descs = &self.descs;
        for hosted in self.embedder.values_mut() {
            let state = &mut hosted.state;
            let (Some(alpha), Some(opacity)) =
                (&mut state.alpha, &descs[state.desc].settings.opacity)
            else {
                continue;
            };
            if alpha.follow(opacity) && state.configured {
                state.role.surface().commit();
            }
        }
    }
}
