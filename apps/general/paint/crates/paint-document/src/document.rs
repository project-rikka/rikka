use std::fs::File;
use std::io::{self, Read};
use std::mem;
use std::path::Path;
use std::sync::Arc;

use rikka_figure::{Affine, Lettering, Measure, Typeset};
use rikka_image::ColorType;
use rikka_image_edit::{
    ConvertProfile, Crop, Depth, Fill, Histogram, Imaging, Intent, Picture, Preview, Profile,
    Recipe, Shown, Step,
};
use rikka_psd::{Compression, FileHeader};
use rikka_psd_render::render_rgba;
use rikka_types::Color;

use crate::area::Area;
use crate::blank::Blank;
use crate::develop::{self, DevelopedLayer};
use crate::document::figuring::{Refit, figured_recipe};
use crate::edit::{Change, Edit, PixelChange, SelectionAction};
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureIds};
use crate::format::FileFormat;
use crate::history::{Action, Focus, History, Limits, Timeline, Travelled};
use crate::layer::{self, Layer, LayerId};
use crate::overlay::Overlays;
use crate::previewing::{self, Previewing};
use crate::reductions::{self, ReductionKind, Reductions};
use crate::region::Region;
use crate::reshape::{self, Reshape};
use crate::save::{self, RENDERED_BYTES_PER_PIXEL, Saved, Saving};
use crate::state::{Content, State, Swap, stale_composite};
use crate::thumbnail::{Canvas, LayerPixels, Thumbnail};
use crate::tile::{self, Tile};
use crate::{convert, crop};

const WORKING_MEMORY: u64 = 2 << 30;
const SHOWN_BYTES_PER_PIXEL: u64 = 4;
const PREVIEW_TILE: u32 = 2048;
const HISTORY: Limits = Limits {
    states: 50,
    bytes: WORKING_MEMORY / 2,
};

#[derive(Debug)]
pub struct Document {
    imaging: Imaging,
    format: FileFormat,
    state: State,
    timeline: Timeline,
    revision: u64,
    previewing: Option<Previewing>,
    sampled: Option<(u64, Picture)>,
    reductions: Reductions,
    on_screen: Option<Shown>,
    gathering: Option<BatchState>,
    figure_ids: FigureIds,
}

struct Recorded {
    action: Action,
    swaps: Vec<Swap>,
    pixels: PixelChange,
}

#[derive(Debug, Default)]
struct BatchState {
    swaps: Vec<Swap>,
    focus: Focus,
    change: Change,
}

impl BatchState {
    fn gather(&mut self, swaps: Vec<Swap>, focus: Focus) {
        self.swaps.extend(swaps);
        self.focus = Focus {
            before: self.focus.before.or(focus.before),
            after: focus.after.or(self.focus.after),
        };
    }
}

impl Document {
    pub fn open(path: &Path, imaging: &Imaging) -> Result<Document> {
        let unreadable = |source: io::Error| Error::Unreadable {
            path: path.to_path_buf(),
            source,
        };
        let mut file = File::open(path).map_err(unreadable)?;
        let mut bytes = Vec::with_capacity(FileHeader::LEN);
        file.by_ref()
            .take(FileHeader::LEN as u64)
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        match FileHeader::is_photoshop(&bytes) {
            true => Document::photoshop(path, file, bytes, imaging),
            false => Document::picture(path, imaging),
        }
    }

    fn photoshop(
        path: &Path,
        mut file: File,
        mut bytes: Vec<u8>,
        imaging: &Imaging,
    ) -> Result<Document> {
        let damaged = |error: rikka_psd::Error| Error::Damaged {
            path: path.to_path_buf(),
            reason: error.to_string(),
        };
        let header = FileHeader::read(&bytes).map_err(damaged)?;
        check_memory_budget(
            header.width,
            header.height,
            header.pixel_count() * SHOWN_BYTES_PER_PIXEL + header.composite_bytes(),
        )?;
        file.read_to_end(&mut bytes)
            .map_err(|source| Error::Unreadable {
                path: path.to_path_buf(),
                source,
            })?;
        let document = rikka_psd::Document::read(&bytes).map_err(damaged)?;
        let format = FileFormat::Photoshop {
            large: header.version.is_large(),
        };
        Ok(Document::holding(
            imaging,
            format,
            Content::Photoshop {
                profile: profile_of(&document),
                document: Box::new(document),
                touched: false,
            },
            Action::Open,
        ))
    }

    pub fn new_blank(blank: &Blank, imaging: &Imaging) -> Result<Document> {
        let document = blank.photoshop()?;
        let format = FileFormat::Photoshop {
            large: document.version == rikka_psd::Version::Psb,
        };
        Ok(Document::holding(
            imaging,
            format,
            Content::Photoshop {
                profile: profile_of(&document),
                document: Box::new(document),
                touched: false,
            },
            Action::New,
        ))
    }

    fn picture(path: &Path, imaging: &Imaging) -> Result<Document> {
        let info = rikka_image::read(path)
            .probe()
            .map_err(|error| picture_error(path, error))?;
        let too_large = || Error::TooLarge {
            width: info.width,
            height: info.height,
        };
        check_memory_budget(
            info.width,
            info.height,
            pixels(info.width, info.height) * SHOWN_BYTES_PER_PIXEL,
        )?;
        let decoded = rikka_image::read(path)
            .format(info.format)
            .decode()
            .map_err(|error| match error {
                rikka_image::Error::LimitsExceeded(_) => too_large(),
                other => picture_error(path, other),
            })?;
        let picture = imaging.picture(&decoded)?;
        Ok(Document::holding(
            imaging,
            FileFormat::Picture(info.format),
            Content::Picture {
                picture,
                visible: true,
            },
            Action::Open,
        ))
    }

    fn holding(imaging: &Imaging, format: FileFormat, content: Content, first: Action) -> Document {
        Document {
            imaging: imaging.clone(),
            format,
            state: State {
                content,
                selection: Region::default(),
                overlays: Overlays::default(),
            },
            timeline: Timeline::new(first, HISTORY),
            revision: 0,
            previewing: None,
            sampled: None,
            reductions: Reductions::default(),
            on_screen: None,
            gathering: None,
            figure_ids: FigureIds::default(),
        }
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.state.width()
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.state.height()
    }

    #[must_use]
    pub fn format(&self) -> FileFormat {
        self.format
    }

    #[must_use]
    pub fn selection(&self) -> &Region {
        &self.state.selection
    }

    #[must_use]
    pub fn history(&self) -> History<'_> {
        self.timeline.view()
    }

    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.timeline.is_modified()
    }

    #[must_use]
    pub fn profile(&self) -> Option<&Profile> {
        match &self.state.content {
            Content::Picture { picture, .. } => picture.profile(),
            Content::Photoshop { profile, .. } => profile.as_ref(),
        }
    }

    #[must_use]
    pub fn is_opaque(&self) -> bool {
        match &self.state.content {
            Content::Picture { picture, .. } => !picture.has_alpha(),
            Content::Photoshop { document, .. } => {
                document.layers.first().is_some_and(layer::is_background)
            }
        }
    }

    #[must_use]
    pub fn layers(&self) -> Vec<Layer> {
        match &self.state.content {
            Content::Picture { visible, .. } => {
                vec![Layer::picture(*visible, &self.state.overlays)]
            }
            Content::Photoshop { document, .. } => {
                Layer::from_psd_nodes(&document.layers, &self.state.overlays)
            }
        }
    }

    pub fn apply(&mut self, edit: &Edit) -> Result<Change> {
        let change = match edit {
            Edit::Undo => self.travel(|timeline, state| timeline.undo(state))?,
            Edit::Redo => self.travel(|timeline, state| timeline.redo(state))?,
            Edit::Revisit(index) => {
                self.travel(|timeline, state| timeline.revisit(*index, state))?
            }
            Edit::Show { layer, visible } => {
                let made = self.show(*layer, *visible);
                self.record(made)
            }
            Edit::Select { region, by } => {
                let made = self.select(region, *by);
                self.record(made)
            }
            Edit::Crop(region) => {
                let made = self.crop(region)?;
                self.record(made)
            }
            Edit::Develop { layer, recipe } => {
                let action = recipe
                    .steps()
                    .last()
                    .map(|step| Action::Develop(step.operation().clone()));
                let made = match action {
                    Some(action) => self.develop(*layer, recipe, action)?,
                    None => None,
                };
                self.record(made)
            }
            Edit::Fill { layer, color } => {
                let made = self.fill(*layer, *color, Action::Fill)?;
                self.record(made)
            }
            Edit::Erase { layer, background } => {
                let color = match self.layer_is_opaque(*layer)? {
                    true => background.with_alpha(1.0),
                    false => Color::TRANSPARENT,
                };
                let made = self.fill(*layer, color, Action::Erase)?;
                self.record(made)
            }
            Edit::Reshape(reshape) => {
                let made = self.reshape(*reshape)?;
                self.record(made)
            }
            Edit::Convert { profile, intent } => {
                let made = self.convert(profile, *intent)?;
                self.record(made)
            }
            Edit::SelectShape { shape, combine } => {
                let made = self.select_shape(shape, *combine);
                self.record(made)
            }
            Edit::MagicWand { flood, combine } => {
                edit.check(&self.layers())?;
                let made = self.magic_wand(flood, *combine)?;
                self.record(made)
            }
            Edit::SelectLayer { layer, combine } => {
                edit.check(&self.layers())?;
                let made = self.select_layer(*layer, *combine)?;
                self.record(made)
            }
            Edit::ModifySelection(modify) => {
                let made = self.modify_selection(*modify)?;
                self.record(made)
            }
            Edit::MoveSelection { dx, dy } => {
                let made = self.move_selection(*dx, *dy);
                self.record(made)
            }
            Edit::Bucket {
                layer,
                flood,
                color,
            } => {
                edit.check(&self.layers())?;
                let made = self.bucket(*layer, flood, *color)?;
                self.record(made)
            }
            Edit::Figures { layer, change } => {
                edit.check(&self.layers())?;
                let made = self.figure(*layer, change)?;
                self.record_focused(made, Focus::on(*layer))
            }
            Edit::Batch { name, edits } => self.batch(name, edits)?,
            Edit::NewLayer { .. }
            | Edit::DuplicateLayer { .. }
            | Edit::DeleteLayer { .. }
            | Edit::MoveLayer { .. }
            | Edit::RenameLayer { .. }
            | Edit::SetOpacity { .. }
            | Edit::SetBlend { .. }
            | Edit::SetProperties { .. }
            | Edit::MergeDown { .. }
            | Edit::MergeVisible
            | Edit::Flatten
            | Edit::LayerFromBackground { .. } => self.arrange(edit)?,
        };
        if change.pixels != PixelChange::Unchanged || change.layers {
            self.revision += 1;
            self.reductions.reached(self.revision);
        }
        if let Some(gathered) = &mut self.gathering {
            gathered.change = gathered.change.and(change);
        }
        Ok(change)
    }

    fn travel(
        &mut self,
        step: impl FnOnce(&mut Timeline, &mut State) -> Result<Option<Travelled>>,
    ) -> Result<Change> {
        let size = (self.width(), self.height());
        let Some(moved) = step(&mut self.timeline, &mut self.state)? else {
            return Ok(Change::NONE);
        };
        self.reductions.touch(moved.touched);
        let pixels = match ((self.width(), self.height()) != size, moved.look) {
            (true, _) => PixelChange::Canvas,
            (false, true) => PixelChange::Content,
            (false, false) => PixelChange::Unchanged,
        };
        Ok(Change {
            pixels,
            selection: moved.selection,
            history: true,
            layers: moved.look,
            layer: moved.layer,
        })
    }

    fn record(&mut self, made: Option<Recorded>) -> Change {
        self.record_focused(made, Focus::default())
    }

    fn record_focused(&mut self, made: Option<Recorded>, focus: Focus) -> Change {
        let Some(made) = made else {
            return Change::NONE;
        };
        let selection = made
            .swaps
            .iter()
            .any(|swap| matches!(swap, Swap::Selection(_)));
        for swap in &made.swaps {
            self.reductions.touch(swap.touched());
        }
        let layers = focus.after.is_some() || matches!(made.action, Action::Show { .. });
        match &mut self.gathering {
            Some(gathered) => gathered.gather(made.swaps, focus),
            None => self.timeline.record(made.action, made.swaps, focus),
        }
        Change {
            pixels: made.pixels,
            selection,
            history: true,
            layers,
            layer: focus.after,
        }
    }

    fn batch(&mut self, name: &str, edits: &[Edit]) -> Result<Change> {
        if edits.iter().any(Edit::travels) {
            return Err(Error::TravelInBatch);
        }
        if self.gathering.is_some() {
            return edits.iter().try_fold(Change::NONE, |change, edit| {
                Ok(change.and(self.apply(edit)?))
            });
        }
        let ((), change) = self.compose(name, |document| {
            edits
                .iter()
                .try_for_each(|edit| document.apply(edit).map(|_| ()))
        })?;
        Ok(change)
    }

    pub fn compose<T, E: From<Error>>(
        &mut self,
        name: &str,
        steps: impl FnOnce(&mut Document) -> std::result::Result<T, E>,
    ) -> std::result::Result<(T, Change), E> {
        if self.gathering.is_some() {
            return Err(Error::NestedBatch.into());
        }
        self.gathering = Some(BatchState::default());
        let composed = steps(self);
        let BatchState {
            mut swaps,
            focus,
            change,
        } = self.gathering.take().unwrap_or_default();
        match composed {
            Ok(made) if swaps.is_empty() => Ok((made, Change::NONE)),
            Ok(made) => {
                self.timeline
                    .record(Action::Batch(name.to_owned()), swaps, focus);
                Ok((
                    made,
                    Change {
                        history: true,
                        ..change
                    },
                ))
            }
            Err(error) => {
                for swap in swaps.iter_mut().rev() {
                    let _ = swap.exchange(&mut self.state);
                    self.reductions.touch(swap.touched());
                }
                self.revision += 1;
                self.reductions.reached(self.revision);
                Err(error)
            }
        }
    }

    fn show(&mut self, id: LayerId, visible_now: bool) -> Option<Recorded> {
        let swap = match &mut self.state.content {
            Content::Picture { visible, .. } => {
                if id != LayerId(0) || *visible == visible_now {
                    return None;
                }
                *visible = visible_now;
                Swap::Shown {
                    layer: id,
                    visible_before: !visible_now,
                }
            }
            Content::Photoshop {
                document, touched, ..
            } => {
                let record = layer::node_mut(document, id)?.layer_mut();
                if record.visible() == visible_now {
                    return None;
                }
                record.set_visible(visible_now);
                *touched = true;
                Swap::Shown {
                    layer: id,
                    visible_before: !visible_now,
                }
            }
        };
        Some(Recorded {
            action: Action::Show {
                layer: id,
                visible: visible_now,
            },
            swaps: vec![swap],
            pixels: PixelChange::Content,
        })
    }

    fn select(&mut self, region: &Region, by: SelectionAction) -> Option<Recorded> {
        let canvas = Area::sized(self.width(), self.height());
        let swap = self.state.reselect(region.clipped(canvas))?;
        Some(Recorded {
            action: Action::Select(by),
            swaps: vec![swap],
            pixels: PixelChange::Unchanged,
        })
    }

    fn crop(&mut self, region: &Region) -> Result<Option<Recorded>> {
        let (width, height) = (self.width(), self.height());
        let canvas = Area::sized(width, height);
        let region = region.clipped(canvas);
        let Some(area) = region.bounds() else {
            return Ok(None);
        };
        let shaped = region.rectangular().is_none();
        if area == canvas && !shaped {
            return Ok(None);
        }
        let outside = match shaped {
            true => Some(region.mask(width, height)?.inverted()),
            false => None,
        };
        let cut = move |_: bool| {
            let mut recipe = Recipe::new();
            if let Some(outside) = &outside {
                recipe.push(Step::new(Fill::new(Color::TRANSPARENT)).masked(outside.clone()));
            }
            recipe.then(Crop::new(area.left, area.top, area.width, area.height))
        };
        let mut swaps = Vec::new();
        match &mut self.state.content {
            Content::Picture { picture, .. } => {
                let cropped = picture.develop(&cut(true))?;
                swaps.push(Swap::Picture(mem::replace(picture, cropped)));
            }
            Content::Photoshop {
                document,
                profile,
                touched,
            } => {
                let mut cropped = document.clone();
                crop::photoshop(&mut cropped, area, shaped.then_some(&region))?;
                swaps.push(Swap::Photoshop {
                    document: mem::replace(document, cropped),
                    profile: profile.clone(),
                    touched: *touched,
                });
                *touched |= shaped;
            }
        }
        self.refit(
            &Refit {
                recipe: &cut,
                map: Affine::translation(-f64::from(area.left), -f64::from(area.top)),
                strokes: false,
                uprights_text: true,
            },
            &mut swaps,
        )?;
        swaps.extend(self.state.reselect(Region::default()));
        Ok(Some(Recorded {
            action: Action::Crop,
            swaps,
            pixels: PixelChange::Canvas,
        }))
    }

    fn develop(
        &mut self,
        id: LayerId,
        recipe: &Recipe,
        action: Action,
    ) -> Result<Option<Recorded>> {
        let mut swaps = Vec::new();
        if let Some((key, _)) = self.overlay_of(id)? {
            if !previewing::reshapes(recipe) {
                return self.develop_overlaid(id, recipe, action);
            }
            let mut overlays = self.state.overlays.clone();
            overlays.set(key, None);
            swaps.push(self.state.overlay(overlays, 0));
        }
        let pixels = match &self.state.content {
            Content::Picture { picture, .. } => {
                if id != LayerId(0) {
                    return Err(no_layer(id));
                }
                let developed = picture.develop(recipe)?;
                let reshaped = developed.dimensions() != picture.dimensions();
                if let Content::Picture { picture, .. } = &mut self.state.content {
                    swaps.extend(Swap::from_pictures(picture, developed)?);
                }
                if reshaped {
                    swaps.extend(self.state.reselect(Region::default()));
                }
                match reshaped {
                    true => PixelChange::Canvas,
                    false => PixelChange::Content,
                }
            }
            Content::Photoshop { .. } => {
                if previewing::reshapes(recipe) {
                    return Err(Error::LayerReshape);
                }
                let placed = self.placed(id)?;
                let developed = placed.picture.develop(recipe)?;
                let Content::Photoshop {
                    document, touched, ..
                } = &mut self.state.content
                else {
                    unreachable!("the content was just matched");
                };
                let record = placed.rewritten_layer(document, &developed, Compression::Rle)?;
                let node = layer::node_mut(document, id).ok_or_else(|| no_layer(id))?;
                swaps.push(Swap::Layer {
                    layer: id,
                    record: Box::new(mem::replace(node.layer_mut(), record)),
                });
                *touched = true;
                PixelChange::Content
            }
        };
        Ok(Some(Recorded {
            action,
            swaps,
            pixels,
        }))
    }

    fn fill(&mut self, id: LayerId, color: Color, action: Action) -> Result<Option<Recorded>> {
        let step = Step::new(Fill::new(color));
        let step = match self.state.selection.is_empty() {
            true => step,
            false => step.masked(self.state.selection.mask(self.width(), self.height())?),
        };
        self.develop(id, &Recipe::new().then(step), action)
    }

    pub fn measure_text(&self, lettering: &Lettering) -> Result<Measure> {
        lettering.check()?;
        Ok(Typeset::new(lettering).measure())
    }

    fn layer_is_opaque(&self, id: LayerId) -> Result<bool> {
        match &self.state.content {
            Content::Picture { picture, .. } => match id == LayerId(0) {
                true => Ok(!picture.has_alpha()),
                false => Err(no_layer(id)),
            },
            Content::Photoshop { document, .. } => {
                Ok(layer::is_opaque_record(develop::pixel_layer(document, id)?))
            }
        }
    }

    fn reshape(&mut self, reshape: Reshape) -> Result<Option<Recorded>> {
        let (width, height) = (self.width(), self.height());
        if reshape.changes_nothing(width, height) {
            return Ok(None);
        }
        let selection = reshape.selection(&self.state.selection, width, height);
        let mut swaps = Vec::new();
        match &mut self.state.content {
            Content::Picture { picture, .. } => {
                let reshaped = picture.develop(&reshape.recipe((width, height)))?;
                swaps.push(Swap::Picture(mem::replace(picture, reshaped)));
            }
            Content::Photoshop {
                document,
                profile,
                touched,
            } => {
                let reshaped = reshape::photoshop(&self.imaging, document, reshape)?;
                swaps.push(Swap::Photoshop {
                    document: mem::replace(document, Box::new(reshaped)),
                    profile: profile.clone(),
                    touched: *touched,
                });
                *touched |= reshape::resamples(reshape);
            }
        }
        let layered = matches!(self.state.content, Content::Photoshop { .. });
        let reshaped = move |opaque: bool| match layered && !opaque {
            true => reshape.cleared().recipe((width, height)),
            false => reshape.recipe((width, height)),
        };
        self.refit(
            &Refit {
                recipe: &reshaped,
                map: Affine::from_array(reshape.map((width, height)).to_array()),
                strokes: reshape.scales(),
                uprights_text: reshape.uprights_text(),
            },
            &mut swaps,
        )?;
        swaps.extend(self.state.reselect(selection));
        Ok(Some(Recorded {
            action: Action::Reshape(reshape),
            swaps,
            pixels: PixelChange::Canvas,
        }))
    }

    fn convert(&mut self, to: &Profile, intent: Intent) -> Result<Option<Recorded>> {
        let previous = self.profile().cloned();
        let swap = match &mut self.state.content {
            Content::Picture { picture, .. } => {
                if picture.profile() == Some(to) {
                    return Ok(None);
                }
                let converted = picture
                    .develop(&Recipe::new().then(ConvertProfile::new(to.clone()).intent(intent)))?;
                Swap::Picture(mem::replace(picture, converted))
            }
            Content::Photoshop {
                document,
                profile,
                touched,
            } => {
                if profile.as_ref() == Some(to) {
                    return Ok(None);
                }
                let converted =
                    convert::photoshop(&self.imaging, document, profile.as_ref(), to, intent)?;
                Swap::Photoshop {
                    document: mem::replace(document, Box::new(converted)),
                    profile: profile.replace(to.clone()),
                    touched: *touched,
                }
            }
        };
        let mut swaps = vec![swap];
        if previous.as_ref() != Some(to) {
            let converted =
                |_: bool| Recipe::new().then(ConvertProfile::new(to.clone()).intent(intent));
            self.refit(
                &Refit {
                    recipe: &converted,
                    map: Affine::IDENTITY,
                    strokes: false,
                    uprights_text: true,
                },
                &mut swaps,
            )?;
        }
        Ok(Some(Recorded {
            action: Action::Convert,
            swaps,
            pixels: PixelChange::Content,
        }))
    }

    fn placed(&self, id: LayerId) -> Result<DevelopedLayer> {
        if let Some(previewing) = &self.previewing
            && previewing.serves(id, self.revision)
            && !previewing.overlaid
            && let Some(placed) = &previewing.placed
        {
            return Ok(placed.clone());
        }
        let Content::Photoshop {
            document, profile, ..
        } = &self.state.content
        else {
            return Err(no_layer(id));
        };
        develop::placed(&self.imaging, document, profile.as_ref(), id)
    }

    pub fn composite(&self) -> Result<Picture> {
        let (width, height) = (self.width(), self.height());
        match &self.state.content {
            Content::Picture { picture, visible } => match visible {
                true => Ok(picture.clone()),
                false => self
                    .imaging
                    .blank(width, height, Depth::Eight, Color::TRANSPARENT)
                    .map_err(Error::from),
            },
            Content::Photoshop {
                document,
                profile,
                touched,
            } => {
                let stale = stale_composite(document, *touched);
                let pixels = match !stale && document.has_real_composite() {
                    true => document.composite_rgba8().map_err(Error::from)?,
                    false => {
                        check_memory_budget(
                            width,
                            height,
                            pixels(width, height) * RENDERED_BYTES_PER_PIXEL,
                        )?;
                        render_rgba(document).map_err(Error::from)?.to_rgba8()
                    }
                };
                let picture = self.imaging.upload_rgba8(width, height, &pixels)?;
                Ok(picture.with_profile(profile.clone()))
            }
        }
    }

    pub fn tiles(&mut self, side: u32, level: u32) -> Result<Vec<Tile>> {
        let picture = self.shown_at(level)?;
        let frame = picture.show(self.on_screen.as_ref())?;
        let tiles = tile::cut(frame.frame(), self.width(), self.height(), side);
        self.on_screen = Some(frame);
        tiles
    }

    fn shown_at(&mut self, level: u32) -> Result<Picture> {
        let (width, height) = (self.width(), self.height());
        let extent = tile::reduced(width, height, level);
        match extent == (width, height) {
            true => self.composite(),
            false => self.reduced(ReductionKind::Shown, extent, Document::composite),
        }
    }

    fn reduced(
        &mut self,
        reduced: ReductionKind,
        extent: (u32, u32),
        source: impl FnOnce(&Document) -> Result<Picture>,
    ) -> Result<Picture> {
        let mut reductions = mem::take(&mut self.reductions);
        let picture = reductions.reduce(reduced, extent, self.revision, || source(self));
        self.reductions = reductions;
        picture
    }

    pub fn preview(&mut self, layer: LayerId, recipe: &Recipe, scale: f32) -> Result<Vec<Tile>> {
        let previewed = self.previewed(layer, recipe, scale)?;
        let frame = previewed.shown()?;
        tile::cut(&frame, self.width(), self.height(), PREVIEW_TILE)
    }

    pub fn preview_figures(
        &mut self,
        layer: LayerId,
        figures: &[Figure],
        scale: f32,
    ) -> Result<Vec<Tile>> {
        let drawn = figured_recipe(&Recipe::new(), figures);
        let rendered = self.rendered(layer, &drawn, scale)?;
        let frame = rendered.shown()?;
        tile::cut(&frame, self.width(), self.height(), PREVIEW_TILE)
    }

    pub(crate) fn previewed(
        &mut self,
        layer: LayerId,
        recipe: &Recipe,
        scale: f32,
    ) -> Result<Picture> {
        if previewing::reshapes(recipe) {
            return Err(Error::CanvasPreview);
        }
        if recipe.is_empty() && matches!(self.state.content, Content::Photoshop { .. }) {
            return self.shown_at(tile::level_for(scale));
        }
        let figures = self.previewing(layer)?.figures.clone();
        let full = figured_recipe(recipe, &figures);
        self.rendered(layer, &full, scale)
    }

    fn rendered(&mut self, layer: LayerId, recipe: &Recipe, scale: f32) -> Result<Picture> {
        let (width, height) = (self.width(), self.height());
        match &self.state.content {
            Content::Picture { visible, .. } => {
                let visible = *visible;
                let rendered = self.previewing(layer)?.preview.render(recipe, scale)?;
                match visible {
                    true => Ok(rendered),
                    false => self
                        .imaging
                        .blank(
                            rendered.width(),
                            rendered.height(),
                            Depth::Eight,
                            Color::TRANSPARENT,
                        )
                        .map_err(Error::from),
                }
            }
            Content::Photoshop { .. } => {
                let composite = self.photoshop_preview(layer, recipe)?;
                let extent = tile::reduced(width, height, tile::level_for(scale));
                reductions::resized(&composite, extent)
            }
        }
    }

    pub fn end_preview(&mut self) {
        self.previewing = None;
    }

    fn previewing(&mut self, layer: LayerId) -> Result<&mut Previewing> {
        let revision = self.revision;
        if !self
            .previewing
            .as_ref()
            .is_some_and(|previewing| previewing.serves(layer, revision))
        {
            let overlay = self.overlay_of(layer)?.map(|(_, overlay)| overlay);
            let (source, placed) = match (&self.state.content, &overlay) {
                (Content::Picture { .. }, _) if layer != LayerId(0) => {
                    return Err(no_layer(layer));
                }
                (Content::Picture { picture, .. }, None) => (picture.clone(), None),
                (Content::Picture { .. }, Some(overlay)) => (overlay.base.clone(), None),
                (
                    Content::Photoshop {
                        document, profile, ..
                    },
                    None,
                ) => {
                    let placed = develop::placed(&self.imaging, document, profile.as_ref(), layer)?;
                    (placed.picture.clone(), Some(placed))
                }
                (Content::Photoshop { document, .. }, Some(overlay)) => {
                    let record = develop::pixel_layer(document, layer)?.clone();
                    let placed = DevelopedLayer::new(record, overlay.base.clone(), overlay.linear);
                    (overlay.base.clone(), Some(placed))
                }
            };
            self.previewing = Some(Previewing {
                layer,
                revision,
                preview: Preview::new(source),
                placed,
                overlaid: overlay.is_some(),
                figures: overlay.map(|overlay| overlay.figures).unwrap_or_default(),
            });
        }
        Ok(self
            .previewing
            .as_mut()
            .unwrap_or_else(|| unreachable!("the preview was just made")))
    }

    fn photoshop_preview(&mut self, layer: LayerId, recipe: &Recipe) -> Result<Picture> {
        let previewing = self.previewing(layer)?;
        let developed = previewing.preview.render(recipe, 1.0)?;
        let placed = previewing
            .placed
            .clone()
            .unwrap_or_else(|| unreachable!("a Photoshop preview holds its layer"));
        let Content::Photoshop {
            document, profile, ..
        } = &mut self.state.content
        else {
            unreachable!("only Photoshop documents preview through rikka-psd-render");
        };
        let record = placed.rewritten_layer(document, &developed, Compression::Raw)?;
        let node = layer::node_mut(document, layer).ok_or_else(|| no_layer(layer))?;
        let previous = mem::replace(node.layer_mut(), record);
        let (width, height) = (document.width, document.height);
        let rendered = check_memory_budget(
            width,
            height,
            pixels(width, height) * RENDERED_BYTES_PER_PIXEL,
        )
        .and_then(|()| render_rgba(document).map_err(Error::from));
        if let Some(node) = layer::node_mut(document, layer) {
            *node.layer_mut() = previous;
        }
        let profile = profile.clone();
        let picture = self
            .imaging
            .upload_rgba8(width, height, &rendered?.to_rgba8())?;
        Ok(picture.with_profile(profile))
    }

    pub fn histogram(&self, layer: LayerId, within: Option<&Region>) -> Result<Histogram> {
        let (width, height) = (self.width(), self.height());
        let picture = self.developed_source(layer)?;
        let canvas = Area::sized(width, height);
        let mut recipe = Recipe::new();
        if let Some(region) = within.map(|region| region.clipped(canvas))
            && let Some(bounds) = region.extent()
        {
            if region.rectangular().is_none() {
                let outside = region.mask(width, height)?.inverted();
                recipe.push(Step::new(Fill::new(Color::TRANSPARENT)).masked(outside));
            }
            if bounds != canvas {
                recipe.push(Crop::new(
                    bounds.left,
                    bounds.top,
                    bounds.width,
                    bounds.height,
                ));
            }
        }
        picture
            .develop(&recipe)
            .and_then(|sampled| sampled.histogram())
            .map_err(Error::from)
    }

    pub fn save(&mut self, path: &Path, saving: &Saving) -> Result<Saved> {
        let format = FileFormat::for_path(path).ok_or_else(|| Error::Unsupported {
            path: path.to_path_buf(),
        })?;
        let (bytes, saved) = match format {
            FileFormat::Photoshop { large } => (self.photoshop_bytes(large)?, Saved::Document),
            FileFormat::Picture(image_format) => {
                let (image, saved) = match &self.state.content {
                    Content::Picture { .. } => {
                        (save::picture_image(&self.composite()?)?, Saved::Document)
                    }
                    Content::Photoshop {
                        document,
                        profile,
                        touched,
                    } => (
                        save::photoshop_image(
                            document,
                            stale_composite(document, *touched),
                            profile.as_ref(),
                            format.keeps_float(),
                        )?,
                        Saved::Copy,
                    ),
                };
                (saving.encode(&image, image_format)?, saved)
            }
        };
        rikka_fs::replace(path, &bytes).map_err(|source| Error::Unwritable {
            path: path.to_path_buf(),
            source,
        })?;
        if saved == Saved::Document {
            self.timeline.mark_saved();
            self.format = format;
        }
        Ok(saved)
    }

    fn photoshop_bytes(&mut self, large: bool) -> Result<Vec<u8>> {
        let result = match &mut self.state.content {
            Content::Picture { picture, visible } => {
                let mut document = save::picture_document(picture, *visible)?;
                save::sized_for(&mut document, large)?;
                document.to_bytes()
            }
            Content::Photoshop {
                document, touched, ..
            } => {
                let stale = stale_composite(document, *touched);
                save::refresh(document, stale)?;
                *touched = stale && !document.has_real_composite();
                save::sized_for(document, large)?;
                document.to_bytes()
            }
        };
        result.map_err(Error::from)
    }

    pub fn thumbnails(&mut self, longest: u32) -> Vec<(LayerId, Thumbnail)> {
        let canvas = Canvas {
            width: self.width(),
            height: self.height(),
        };
        match &self.state.content {
            Content::Picture { .. } => self
                .picture_thumbnail(longest)
                .map(|thumbnail| vec![(LayerId(0), thumbnail)])
                .unwrap_or_default(),
            Content::Photoshop { document, .. } => layer::nodes(document)
                .filter_map(|(id, node)| match node {
                    rikka_psd::Node::Layer(layer) => {
                        Some((id, layer_thumbnail(document, layer, &canvas, longest)?))
                    }
                    rikka_psd::Node::Group(_) => None,
                })
                .collect(),
        }
    }

    pub fn thumbnail(&mut self, layer: LayerId, longest: u32) -> Option<Thumbnail> {
        let canvas = Canvas {
            width: self.width(),
            height: self.height(),
        };
        match &self.state.content {
            Content::Picture { .. } => (layer == LayerId(0))
                .then(|| self.picture_thumbnail(longest))
                .flatten(),
            Content::Photoshop { document, .. } => match layer::node(document, layer)? {
                rikka_psd::Node::Layer(record) => {
                    layer_thumbnail(document, record, &canvas, longest)
                }
                rikka_psd::Node::Group(_) => None,
            },
        }
    }

    fn picture_thumbnail(&mut self, longest: u32) -> Option<Thumbnail> {
        let canvas = Canvas {
            width: self.width(),
            height: self.height(),
        };
        let (width, height) = canvas.fitted(longest);
        let picture = |document: &Document| document.layer_picture(LayerId(0));
        let shrunk = match (width, height) == (canvas.width, canvas.height) {
            true => picture(self),
            false => self.reduced(ReductionKind::Thumbnail, (width, height), picture),
        }
        .ok()?;
        let pixels = shrunk
            .to_image()
            .ok()?
            .convert(ColorType::Rgba8)
            .as_bytes()
            .to_vec();
        Some(Thumbnail {
            width,
            height,
            pixels,
        })
    }
}

fn no_layer(id: LayerId) -> Error {
    Error::UnknownLayer(id)
}

fn profile_of(document: &rikka_psd::Document) -> Option<Profile> {
    let bytes = document.resources.icc_profile()?;
    Profile::from_icc(Arc::<[u8]>::from(bytes)).ok()
}

fn layer_thumbnail(
    document: &rikka_psd::Document,
    layer: &rikka_psd::Layer,
    canvas: &Canvas,
    longest: u32,
) -> Option<Thumbnail> {
    if layer.kind() == rikka_psd::LayerKind::Adjustment {
        return None;
    }
    if layer.bounds.is_empty() {
        let nothing = |_: usize| [0.0; 4];
        return Some(canvas.shrink(
            longest,
            &LayerPixels {
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                pixel: &nothing,
            },
        ));
    }
    let bounds = layer.bounds;
    check_memory_budget(
        bounds.width(),
        bounds.height(),
        bounds.area() * develop::DECODED_BYTES_PER_PIXEL,
    )
    .ok()?;
    let image = document.layer_rgba(layer).ok()?;
    let encode = |value: f32| match image.linear {
        true => rikka_types::srgb::from_linear_f64(f64::from(value)) as f32,
        false => value,
    };
    let pixel = |index: usize| {
        let [r, g, b, a] = image.pixels[index];
        [encode(r), encode(g), encode(b), a]
    };
    let placed = LayerPixels {
        left: layer.bounds.left,
        top: layer.bounds.top,
        width: image.width,
        height: image.height,
        pixel: &pixel,
    };
    Some(canvas.shrink(longest, &placed))
}

fn picture_error(path: &Path, error: rikka_image::Error) -> Error {
    match error {
        rikka_image::Error::Io(source) => Error::Unreadable {
            path: path.to_path_buf(),
            source,
        },
        rikka_image::Error::UnknownFormat
        | rikka_image::Error::UnknownExtension(_)
        | rikka_image::Error::Unsupported(_) => Error::Unsupported {
            path: path.to_path_buf(),
        },
        other => Error::Damaged {
            path: path.to_path_buf(),
            reason: other.to_string(),
        },
    }
}

fn pixels(width: u32, height: u32) -> u64 {
    u64::from(width) * u64::from(height)
}

pub(crate) fn check_memory_budget(width: u32, height: u32, bytes: u64) -> Result<()> {
    match bytes <= WORKING_MEMORY {
        true => Ok(()),
        false => Err(Error::TooLarge { width, height }),
    }
}

mod figuring;
mod layering;
mod makeup;
mod sample;
mod selecting;
