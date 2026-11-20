use std::rc::Rc;
use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_server::Client;

use crate::committed::{BufferChange, Changes, Committed};
use crate::current::Attached;
use crate::dmabuf::pending_fences;
use crate::ids::{SurfaceId, UseId};
use crate::state::{Placement, Update};
use crate::surfaces::Surfaces;
use crate::syncobj::SyncPoints;
use crate::transaction::{Entries, Transaction};

const MAX_WAITING: usize = 256;

const MAX_SPARE: usize = 32;

impl Surfaces {
    pub(crate) fn take_pending(&mut self, id: SurfaceId) -> Option<Box<Update>> {
        let surface = self.surfaces.get_mut(id)?;
        surface.latest = surface.latest.after(&surface.pending, &self.buffers);
        let spare = self.spare.pop().unwrap_or_default();
        let pending = &mut surface.pending;
        let mut update = std::mem::replace(&mut pending.update, spare);
        let points = std::mem::take(&mut pending.points);
        let release_callbacks = std::mem::take(&mut pending.release_callbacks);
        update.buffer = pending.buffer.take().map(|attached| {
            attached.and_then(|buffer| self.buffers.begin_use(buffer, release_callbacks))
        });
        if let Some(Some(used)) = update.buffer {
            self.synchronize(id, used, points, &mut update);
        }
        Some(update)
    }
    fn synchronize(&mut self, id: SurfaceId, used: UseId, points: SyncPoints, update: &mut Update) {
        if let Some(release) = points.release {
            self.buffers.set_release_point(used, release);
        }
        match points.acquire {
            Some(acquire) if acquire.signaled() => {}
            Some(acquire) => update.fences.extend(self.blockers.acquire(id, acquire)),
            None => {
                let fences = self
                    .buffers
                    .used_buffer(used)
                    .and_then(|buffer| self.buffers.dmabuf(buffer))
                    .map(pending_fences)
                    .unwrap_or_default();
                for fence in fences {
                    update.fences.push(self.blockers.readable(fence));
                }
            }
        }
    }

    pub(crate) fn cache(&mut self, id: SurfaceId, mut update: Box<Update>) {
        let Some(surface) = self.surfaces.get_mut(id) else {
            update.discard(&mut self.buffers, &mut self.blockers);
            return self.recycle(update);
        };
        match surface.cached.as_mut() {
            Some(cached) => {
                cached.absorb(&mut update, &mut self.buffers, &mut self.blockers);
                self.recycle(update);
            }
            None => surface.cached = Some(update),
        }
    }

    pub(crate) fn enqueue(&mut self, id: SurfaceId, mut update: Box<Update>) {
        let Some(surface) = self.surfaces.get_mut(id) else {
            update.discard(&mut self.buffers, &mut self.blockers);
            return self.recycle(update);
        };
        let update = match surface.cached.take() {
            Some(mut cached) => {
                cached.absorb(&mut update, &mut self.buffers, &mut self.blockers);
                self.recycle(update);
                cached
            }
            None => update,
        };
        let mut entries = Entries::new();
        entries.push((id, update));
        self.gather_synced(id, &mut entries);
        if let Some(flooded) = entries
            .iter()
            .find(|(surface, _)| self.queue.waiting_on(*surface) >= MAX_WAITING)
            .and_then(|(surface, _)| self.surfaces.get(*surface))
            .and_then(|surface| Client::from_resource(&surface.resource))
        {
            flooded.kill("too many commits wait on one surface");
            for (_, mut update) in entries {
                update.discard(&mut self.buffers, &mut self.blockers);
                self.recycle(update);
            }
            return;
        }
        let blockers = &self.blockers;
        self.queue
            .push(entries, |blocker| blockers.is_active(blocker));
    }

    pub(crate) fn enqueue_cached(&mut self, id: SurfaceId) {
        if let Some(cached) = self
            .surfaces
            .get_mut(id)
            .and_then(|surface| surface.cached.take())
        {
            self.enqueue(id, cached);
        }
    }

    fn gather_synced(&mut self, parent: SurfaceId, entries: &mut Entries) {
        let mut walk = std::mem::take(&mut self.walk);
        if let Some(surface) = self.surfaces.get(parent) {
            walk.extend(surface.children.iter().rev().map(|child| (*child, false)));
        }
        while let Some((child, all)) = walk.pop() {
            let Some(surface) = self.surfaces.get_mut(child) else {
                continue;
            };
            if !all && !surface.sync {
                continue;
            }
            if let Some(cached) = surface.cached.take() {
                entries.push((child, cached));
            }
            walk.extend(
                surface
                    .children
                    .iter()
                    .rev()
                    .map(|grandchild| (*grandchild, true)),
            );
        }
        self.walk = walk;
    }

    pub(crate) fn apply_next(&mut self, commits: &mut Vec<Committed>) -> bool {
        let now = Timestamp::now().as_monotonic();
        while let Some(id) = self.queue.next_candidate() {
            let unmet = self
                .queue
                .get(id)
                .and_then(|transaction| self.unmet_by(transaction, now));
            if let Some(deadline) = unmet {
                self.queue.constrain(id, deadline);
                continue;
            }
            let Some(Transaction { entries, .. }) = self.queue.take(id) else {
                continue;
            };
            for (surface, mut update) in entries {
                if let Some(committed) = self.apply(surface, &mut update) {
                    commits.push(committed);
                }
                self.recycle(update);
            }
            return true;
        }
        false
    }

    fn unmet_by(&self, transaction: &Transaction, now: Duration) -> Option<Option<Duration>> {
        let mut unmet = None;
        for (surface, update) in &transaction.entries {
            if self.unmet(*surface, &update.constraints, now).is_none() {
                continue;
            }
            let early = update.constraints.target.filter(|target| *target > now);
            unmet = Some(match (unmet.flatten(), early) {
                (Some(remaining), Some(early)) => Some(Duration::min(remaining, early)),
                (remaining, early) => remaining.or(early),
            });
        }
        unmet
    }

    pub(crate) fn recycle(&mut self, mut update: Box<Update>) {
        if self.spare.len() < MAX_SPARE {
            update.reset();
            self.spare.push(update);
        }
    }

    fn apply(&mut self, id: SurfaceId, update: &mut Update) -> Option<Committed> {
        let Some(surface) = self.surfaces.get_mut(id) else {
            update.discard(&mut self.buffers, &mut self.blockers);
            return None;
        };
        let supersedes_feedback = update.supersedes_feedback();
        self.blockers.release_all(update.fences.drain(..));
        self.blockers.release_all(update.blockers.drain(..));
        let current = &mut surface.current;
        let before = current.geometry();
        let was_mapped = current.buffer.is_some();
        let mut changes = Changes::empty();

        let buffer = match update.buffer.take() {
            None => BufferChange::Unchanged,
            Some(next) => {
                let attached = next.and_then(|used| {
                    let (buffer, size) = self.buffers.used(used)?;
                    Some(Attached {
                        buffer,
                        size,
                        use_id: Some(used),
                    })
                });
                if let Some(Attached {
                    use_id: Some(use_id),
                    ..
                }) = std::mem::replace(&mut current.buffer, attached)
                {
                    self.buffers.drop_hold(use_id);
                }
                if attached.is_some() {
                    BufferChange::Attached
                } else {
                    BufferChange::Removed
                }
            }
        };
        changes |= latch(&mut current.scale, update.scale, Changes::SCALE)
            | latch(&mut current.transform, update.transform, Changes::TRANSFORM)
            | latch(&mut current.source, update.source, Changes::VIEWPORT)
            | latch(
                &mut current.destination,
                update.destination,
                Changes::VIEWPORT,
            )
            | latch(&mut current.alpha, update.alpha, Changes::ALPHA)
            | latch(
                &mut current.content_type,
                update.content_type,
                Changes::CONTENT_TYPE,
            )
            | latch(&mut current.tearing, update.tearing, Changes::TEARING);
        if let Some(opaque) = update.opaque.take() {
            current.opaque = opaque;
            changes |= Changes::OPAQUE;
        }
        if let Some(input) = update.input.take() {
            current.input = input;
            changes |= Changes::INPUT;
        }
        if update.offset != Point::zero() {
            changes |= Changes::OFFSET;
        }
        if update.color.apply(&mut current.color) {
            changes |= Changes::COLOR;
        }
        let barrier_set = update.constraints.set_barrier && !current.barrier;
        if update.constraints.set_barrier {
            current.barrier = true;
        }

        let after = current.geometry();
        let mapping = after.mapping();
        current.size = mapping.map(|mapping| mapping.size());
        if before.size() != current.size {
            changes |= Changes::SIZE;
        }
        if buffer != BufferChange::Unchanged || changes.contains(Changes::SIZE) {
            let filled = current
                .buffer
                .filter(|attached| self.buffers.opaque(attached.buffer))
                .and(current.size)
                .map(|size| {
                    let mut whole = Region::new();
                    whole.add(Rect::from_size(size));
                    Rc::new(whole)
                });
            if filled.as_deref() != current.filled.as_deref() {
                current.filled = filled;
                changes |= Changes::OPAQUE;
            }
        }
        if let Some(mapping) = mapping {
            if before != after {
                current.damage.buffer.add(Rect::from_size(mapping.buffer()));
                current.damage.surface.add(Rect::from_size(mapping.size()));
                changes |= Changes::DAMAGE;
            } else {
                for rect in update.damage.iter() {
                    if let Some(converted) = mapping.surface_to_buffer(rect) {
                        current.damage.buffer.add(converted);
                        current.damage.surface.add(clip(rect, mapping.size()));
                        changes |= Changes::DAMAGE;
                    }
                }
                for rect in update.buffer_damage.iter() {
                    if let Some(converted) = mapping.buffer_to_surface(rect) {
                        current.damage.surface.add(converted);
                        if let Some(clipped) = rect.intersection(Rect::from_size(mapping.buffer()))
                        {
                            current.damage.buffer.add(clipped);
                        }
                        changes |= Changes::DAMAGE;
                    }
                }
            }
        }

        if !update.frame_callbacks.is_empty() {
            current.frame_callbacks.append(&mut update.frame_callbacks);
            changes |= Changes::FRAME_CALLBACKS;
        }
        if supersedes_feedback {
            std::mem::swap(&mut current.feedback, &mut update.feedback);
            for superseded in update.feedback.drain(..) {
                superseded.discarded();
            }
        }
        if !current.feedback.is_empty() {
            changes |= Changes::FEEDBACK;
        }
        let applied = &mut self.applied;
        current
            .cached
            .apply(&mut update.cached, |hook| applied.push((id, hook)));
        current.commits += 1;
        let sequence = current.commits;
        let remapped = was_mapped != current.buffer.is_some();
        if barrier_set {
            self.barriers.push(id);
        }
        if self.place_children(id, update) {
            changes |= Changes::CHILDREN;
        }
        if remapped || changes.intersects(Changes::SIZE | Changes::CHILDREN) {
            self.invalidate_extents(id);
        }
        Some(Committed {
            surface: id,
            sequence,
            buffer,
            offset: update.offset,
            changes,
        })
    }

    fn place_children(&mut self, parent: SurfaceId, update: &mut Update) -> bool {
        let mut changed = false;
        for (child, position) in update.positions.drain(..) {
            if let Some(surface) = self.surfaces.get_mut(child)
                && surface.parent == Some(parent)
            {
                changed |= surface.position != position;
                surface.position = position;
            }
        }
        for placement in update.placements.drain(..) {
            changed |= self.place(parent, placement);
        }
        changed
    }

    fn place(&mut self, parent: SurfaceId, placement: Placement) -> bool {
        let (child, anchor) = match placement {
            Placement::Add(child) => (child, None),
            Placement::Above { child, sibling } => (child, Some((sibling, 1))),
            Placement::Below { child, sibling } => (child, Some((sibling, 0))),
        };
        if self.surfaces.get(child).and_then(|surface| surface.parent) != Some(parent) {
            return false;
        }
        let Some(stack) = self
            .surfaces
            .get_mut(parent)
            .map(|surface| &mut surface.stack)
        else {
            return false;
        };
        match anchor {
            None if stack.contains(&child) => false,
            None => {
                stack.push(child);
                true
            }
            Some((sibling, after)) => {
                if sibling == child || !stack.contains(&sibling) {
                    return false;
                }
                stack.retain(|entry| *entry != child);
                let Some(index) = stack.iter().position(|entry| *entry == sibling) else {
                    return false;
                };
                stack.insert(index + after, child);
                true
            }
        }
    }
}

fn latch<T: PartialEq>(current: &mut T, next: Option<T>, change: Changes) -> Changes {
    match next {
        Some(next) if next != *current => {
            *current = next;
            change
        }
        _ => Changes::empty(),
    }
}

fn clip(rect: Rect<i32, Logical>, size: Size<i32, Logical>) -> Rect<i32, Logical> {
    rect.intersection(Rect::from_size(size)).unwrap_or(rect)
}
