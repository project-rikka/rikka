use crate::window::XWindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    Above(XWindowId),
    Below(XWindowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Restack {
    pub(crate) window: XWindowId,
    pub(crate) placement: Placement,
}

#[derive(Debug, Default)]
pub(crate) struct Stacking {
    clients: Vec<XWindowId>,
    order: Vec<XWindowId>,
}

impl Stacking {
    pub(crate) fn clients(&self) -> &[XWindowId] {
        &self.clients
    }

    pub(crate) fn order(&self) -> &[XWindowId] {
        &self.order
    }

    pub(crate) fn mapped(&mut self, window: XWindowId) -> bool {
        if self.clients.contains(&window) {
            return false;
        }
        self.clients.push(window);
        self.order.push(window);
        true
    }

    pub(crate) fn unmapped(&mut self, window: XWindowId) -> bool {
        let known = self.clients.contains(&window);
        self.clients.retain(|client| *client != window);
        self.order.retain(|client| *client != window);
        known
    }

    pub(crate) fn raise(&mut self, window: XWindowId) -> Vec<Restack> {
        let mut order: Vec<XWindowId> = self
            .order
            .iter()
            .copied()
            .filter(|client| *client != window)
            .collect();
        order.push(window);
        self.restack(&order)
    }

    pub(crate) fn restack(&mut self, order: &[XWindowId]) -> Vec<Restack> {
        let target = self.target(order);
        let positions: Vec<usize> = target
            .iter()
            .map(|window| {
                self.order
                    .iter()
                    .position(|client| client == window)
                    .unwrap_or_default()
            })
            .collect();
        let staying = longest_increasing(&positions);
        let anchor = staying.iter().position(|stays| *stays);
        let mut moves = Vec::new();
        for (index, window) in target.iter().enumerate() {
            if staying[index] {
                continue;
            }
            let placement = match index.checked_sub(1) {
                Some(below) => Placement::Above(target[below]),
                None => match anchor {
                    Some(anchor) => Placement::Below(target[anchor]),
                    None => continue,
                },
            };
            moves.push(Restack {
                window: *window,
                placement,
            });
        }
        self.order = target;
        moves
    }

    fn target(&self, order: &[XWindowId]) -> Vec<XWindowId> {
        let mut named: Vec<XWindowId> = Vec::with_capacity(order.len());
        for window in order {
            if self.order.contains(window) && !named.contains(window) {
                named.push(*window);
            }
        }
        let mut target: Vec<XWindowId> = self
            .order
            .iter()
            .copied()
            .filter(|window| !named.contains(window))
            .collect();
        target.extend(named);
        target
    }
}

fn longest_increasing(values: &[usize]) -> Vec<bool> {
    let mut length = vec![1usize; values.len()];
    let mut previous: Vec<Option<usize>> = vec![None; values.len()];
    for current in 0..values.len() {
        for earlier in 0..current {
            if values[earlier] < values[current] && length[earlier] + 1 > length[current] {
                length[current] = length[earlier] + 1;
                previous[current] = Some(earlier);
            }
        }
    }
    let mut chosen = vec![false; values.len()];
    let mut cursor = (0..values.len()).max_by_key(|index| (length[*index], usize::MAX - index));
    while let Some(index) = cursor {
        chosen[index] = true;
        cursor = previous[index];
    }
    chosen
}
