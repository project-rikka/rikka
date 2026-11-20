use crate::cloth::Cloth;
use crate::figure::Figure;
use crate::layout::{FigureRecord, Group, JobRecord, NONE};
use crate::step::StepPlan;
use crate::step::round::Environment;

pub(crate) struct Context<'a> {
    pub(crate) environment: &'a Environment,
    pub(crate) table: u64,
    pub(crate) index: u32,
    pub(crate) figures: u32,
    pub(crate) epoch: u32,
    pub(crate) stats: u32,
}

impl Cloth {
    pub(crate) fn table_record(&self, figure: &Figure, epoch: u32) -> FigureRecord {
        let sections = figure.sections;
        let posed = figure.posed_layout;
        FigureRecord {
            positions: figure.state.address(),
            epoch_start: figure.posed_at(posed.epoch_start),
            rest: figure.statics_at(sections.rest),
            traits: figure.statics_at(sections.traits),
            triangles: figure.statics_at(sections.triangles),
            pieces: figure.frame.address() + figure.frame_layout.pieces,
            body_from: figure.body_at(epoch),
            body_to: figure.body_at(epoch + 1),
            body_triangles: figure.statics_at(sections.body_triangles),
            incident_starts: figure.statics_at(sections.incident_starts),
            incident: figure.statics_at(sections.incident),
            epoch_motion: figure.posed_at(posed.epoch_motion),
            counts: [
                figure.count(),
                figure.topology.triangles.len() as u32,
                figure.body_vertices(),
                figure.body_triangles(),
            ],
        }
    }

    fn base_job(&self, figure: &Figure, step_plan: &StepPlan, context: &Context<'_>) -> JobRecord {
        let sections = figure.sections;
        let scratch = figure.scratch_layout;
        let posed = figure.posed_layout;
        let epoch = context.epoch;
        let epochs = self.settings.effort.epochs;
        let per_epoch = step_plan.substeps / epochs;
        let floor = self.settings.floor;
        let h = 1.0 / (self.settings.rate * step_plan.substeps as f32);
        let environment = context.environment;
        JobRecord {
            positions: figure.state.address(),
            velocities: figure.state.address() + figure.velocities(),
            drift: figure.state.address() + figure.drift(),
            starts: figure.scratch_at(scratch.starts),
            preds: figure.scratch_at(scratch.preds),
            epoch_start: figure.posed_at(posed.epoch_start),
            targets_from: figure.targets_at(epoch),
            targets_to: figure.targets_at(epoch + 1),
            rest: figure.statics_at(sections.rest),
            traits: figure.statics_at(sections.traits),
            edges: figure.statics_at(sections.edges),
            bends: figure.statics_at(sections.bends),
            shapes: figure.statics_at(sections.shapes),
            tether_starts: figure.statics_at(sections.tether_starts),
            tethers: figure.statics_at(sections.tethers),
            plan: figure.statics_at(sections.plan),
            triangles: figure.statics_at(sections.triangles),
            incident_starts: figure.statics_at(sections.incident_starts),
            incident: figure.statics_at(sections.incident),
            normals: figure.normals.address(),
            pieces: figure.frame.address() + figure.frame_layout.pieces,
            stats: self.stats.address(),
            slots: figure.scratch_at(scratch.slots),
            gather_starts: figure.statics_at(sections.gather_starts),
            wrap_dests: figure.statics_at(sections.wrap_dests),
            geometry: figure.scratch_at(scratch.geometry),
            contacts: figure.contacts.address(),
            wraps: figure.scratch_at(scratch.wraps),
            selfs: figure.scratch_at(scratch.selfs),
            figures: context.table,
            body_from: figure.body_at(epoch),
            body_to: figure.body_at(epoch + 1),
            body_triangles: figure.statics_at(sections.body_triangles),
            colliders_from: figure.colliders_at(epoch),
            colliders_to: figure.colliders_at(epoch + 1),
            planes_from: figure.planes_at(epoch),
            planes_to: figure.planes_at(epoch + 1),
            cells: self.grid.cells.address(),
            items: self.grid.items.address(),
            spheres: self.grid.spheres.address(),
            large: self.grid.large.address(),
            nearest: figure.scratch_at(scratch.nearest),
            epoch_motion: figure.posed_at(posed.epoch_motion),
            grabbed: figure.grabbed(),
            grabs_from: figure.grabs_at(epoch),
            grabs_to: figure.grabs_at(epoch + 1),
            range: [0, figure.count(), 0, figure.topology.triangles.len() as u32],
            colours: [0; 4],
            items_range: [0; 4],
            shapes_figure: [0, 0, context.index, figure.colliders.len() as u32],
            steps: [
                epoch * per_epoch,
                per_epoch,
                step_plan.substeps,
                if epoch + 1 == epochs {
                    context.stats
                } else {
                    NONE
                },
            ],
            flags: [
                u32::from(floor.is_some()),
                u32::from(epoch == 0),
                context.figures,
                step_plan.grabbed,
            ],
            step: [
                h,
                environment.time,
                floor.unwrap_or(0.0),
                self.settings.epoch_span(),
            ],
            gravity: self.settings.gravity.extend(0.0).to_array(),
            wind: environment.wind.extend(environment.turbulence).to_array(),
            rest_terms: [
                step_plan.still_damping,
                step_plan.activity,
                step_plan.stiffening,
                step_plan.freeze,
            ],
        }
    }

    pub(crate) fn probe_job(
        &self,
        figure: &Figure,
        step_plan: &StepPlan,
        context: &Context<'_>,
    ) -> JobRecord {
        self.base_job(figure, step_plan, context)
    }

    pub(crate) fn solve_job(
        &self,
        figure: &Figure,
        step_plan: &StepPlan,
        group: &Group,
        context: &Context<'_>,
    ) -> JobRecord {
        JobRecord {
            range: [
                group.particles.first,
                group.particles.count,
                group.triangles.first,
                group.triangles.count,
            ],
            colours: [
                group.plan,
                group.structural_colours,
                group.stretch_colours,
                group.inner_end,
            ],
            items_range: [group.bends.first, group.bends.count, 0, 0],
            shapes_figure: [
                group.shapes.first,
                group.shapes.count,
                context.index,
                figure.colliders.len() as u32,
            ],
            ..self.base_job(figure, step_plan, context)
        }
    }
}

pub(crate) fn table_offset(table: u64, round_figures: usize, epoch: u32) -> u64 {
    table + u64::from(epoch) * round_figures as u64 * size_of::<FigureRecord>() as u64
}
