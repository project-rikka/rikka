use std::mem;

use rikka_image_edit::Profile;

use super::figuring::FigureOrigin;
use super::{Document, Recorded, no_layer, profile_of};
use crate::edit::{Change, Edit, PixelChange};
use crate::error::Result;
use crate::history::{Action, Focus};
use crate::layer::{self, LayerId};
use crate::overlay::Overlays;
use crate::restyle::Restyle;
use crate::state::{Content, Swap};
use crate::{arrange, merge, save};

impl Document {
    pub(super) fn arrange(&mut self, edit: &Edit) -> Result<Change> {
        edit.check(&self.layers())?;
        if let (Some(restyle), Some(id)) = (Restyle::from_edit(edit), restyled(edit)) {
            let made = self.restyle(id, &restyle)?;
            return Ok(self.focused(made));
        }
        let made = match edit {
            Edit::NewLayer { above, name } => self.restructure(
                Action::NewLayer,
                Some(*above),
                FigureOrigin::Kept,
                |document| arrange::add(document, *above, name).map(Some),
            )?,
            Edit::DuplicateLayer { layer, name } => self.restructure(
                Action::DuplicateLayer,
                Some(*layer),
                FigureOrigin::Copied { from: *layer },
                |document| arrange::duplicate(document, *layer, name).map(Some),
            )?,
            Edit::DeleteLayer { layer } => self.restructure(
                Action::DeleteLayer,
                Some(*layer),
                FigureOrigin::Kept,
                |document| arrange::delete(document, *layer).map(Some),
            )?,
            Edit::MoveLayer { layer, to } => self.restructure(
                Action::MoveLayer,
                Some(*layer),
                FigureOrigin::Kept,
                |document| arrange::shift(document, *layer, *to),
            )?,
            Edit::MergeDown { layer } => self.restructure(
                Action::MergeDown,
                Some(*layer),
                FigureOrigin::Merged,
                |document| merge::down(document, *layer).map(Some),
            )?,
            Edit::MergeVisible => self.restructure(
                Action::MergeVisible,
                None,
                FigureOrigin::Merged,
                |document| merge::visible(document).map(Some),
            )?,
            Edit::Flatten => {
                self.restructure(Action::Flatten, None, FigureOrigin::Merged, |document| {
                    merge::flatten(document).map(Some)
                })?
            }
            Edit::LayerFromBackground { name } => self.restructure(
                Action::LayerFromBackground,
                None,
                FigureOrigin::Unlocked,
                |document| arrange::unbackground(document, name).map(Some),
            )?,
            _ => unreachable!("{edit:?} does not arrange layers"),
        };
        Ok(self.focused(made))
    }

    fn focused(&mut self, made: Option<(Recorded, Focus)>) -> Change {
        match made {
            Some((made, focus)) => self.record_focused(Some(made), focus),
            None => Change::NONE,
        }
    }

    fn restyle(&mut self, id: LayerId, restyle: &Restyle) -> Result<Option<(Recorded, Focus)>> {
        let pixels = match restyle {
            Restyle::Name(_) => PixelChange::Unchanged,
            Restyle::Opacity(_) | Restyle::Blend(_) | Restyle::Properties(_) => {
                PixelChange::Content
            }
        };
        let Content::Photoshop {
            document, touched, ..
        } = &mut self.state.content
        else {
            return self.restructure(restyle.action(), Some(id), FigureOrigin::Kept, |document| {
                let node = layer::node_mut(document, id).ok_or_else(|| no_layer(id))?;
                Ok(restyle.apply(node).then_some(id))
            });
        };
        let node = layer::node_mut(document, id).ok_or_else(|| no_layer(id))?;
        let record = node.layer().clone();
        if !restyle.apply(node) {
            return Ok(None);
        }
        *touched |= pixels != PixelChange::Unchanged;
        Ok(Some((
            Recorded {
                action: restyle.action(),
                swaps: vec![Swap::Layer {
                    layer: id,
                    record: Box::new(record),
                }],
                pixels,
            },
            Focus::on(id),
        )))
    }

    fn restructure(
        &mut self,
        action: Action,
        before: Option<LayerId>,
        carried: FigureOrigin,
        change: impl FnOnce(&mut rikka_psd::Document) -> Result<Option<LayerId>>,
    ) -> Result<Option<(Recorded, Focus)>> {
        let (mut working, profile, overlays) = self.layered()?;
        let Some(after) = change(&mut working)? else {
            return Ok(None);
        };
        let overlays = self.carried(overlays, &working, &carried, after)?;
        let reoverlaid = !(overlays.is_empty() && self.state.overlays.is_empty());
        let swap = match &mut self.state.content {
            Content::Picture { .. } => Swap::Content(Box::new(mem::replace(
                &mut self.state.content,
                Content::Photoshop {
                    document: Box::new(working),
                    profile,
                    touched: true,
                },
            ))),
            Content::Photoshop {
                document,
                profile,
                touched,
            } => {
                let swap = Swap::Photoshop {
                    document: mem::replace(document, Box::new(working)),
                    profile: profile.clone(),
                    touched: *touched,
                };
                *touched = true;
                swap
            }
        };
        let mut swaps = vec![swap];
        if reoverlaid {
            swaps.push(self.state.overlay(overlays, 0));
        }
        Ok(Some((
            Recorded {
                action,
                swaps,
                pixels: PixelChange::Content,
            },
            Focus {
                before,
                after: Some(after),
            },
        )))
    }

    fn layered(&self) -> Result<(rikka_psd::Document, Option<Profile>, Overlays)> {
        match &self.state.content {
            Content::Picture { picture, visible } => {
                let mut document = save::picture_document(picture, *visible)?;
                let profile = profile_of(&document);
                let overlays = self.layered_overlays(&mut document);
                Ok((document, profile, overlays))
            }
            Content::Photoshop {
                document, profile, ..
            } => Ok((
                (**document).clone(),
                profile.clone(),
                self.state.overlays.clone(),
            )),
        }
    }
}

fn restyled(edit: &Edit) -> Option<LayerId> {
    match edit {
        Edit::RenameLayer { layer, .. }
        | Edit::SetOpacity { layer, .. }
        | Edit::SetBlend { layer, .. }
        | Edit::SetProperties { layer, .. } => Some(*layer),
        _ => None,
    }
}
