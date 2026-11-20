#[derive(Clone, Debug)]
pub(crate) struct Union {
    parent: Vec<u32>,
}

impl Union {
    pub(crate) fn new(count: usize) -> Union {
        Union {
            parent: (0..count as u32).collect(),
        }
    }

    pub(crate) fn find(&mut self, item: u32) -> u32 {
        let mut root = item;
        while self.parent[root as usize] != root {
            root = self.parent[root as usize];
        }
        let mut walk = item;
        while self.parent[walk as usize] != root {
            let next = self.parent[walk as usize];
            self.parent[walk as usize] = root;
            walk = next;
        }
        root
    }

    pub(crate) fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        match a.cmp(&b) {
            std::cmp::Ordering::Less => self.parent[b as usize] = a,
            std::cmp::Ordering::Greater => self.parent[a as usize] = b,
            std::cmp::Ordering::Equal => {}
        }
    }

    pub(crate) fn labels(&mut self) -> (Vec<u32>, usize) {
        let mut label = vec![u32::MAX; self.parent.len()];
        let mut count = 0u32;
        let mut labels = Vec::with_capacity(self.parent.len());
        for item in 0..self.parent.len() as u32 {
            let root = self.find(item) as usize;
            if label[root] == u32::MAX {
                label[root] = count;
                count += 1;
            }
            labels.push(label[root]);
        }
        (labels, count as usize)
    }
}

pub(crate) fn groups(labels: &[u32], count: usize) -> Vec<Vec<u32>> {
    let mut groups = vec![Vec::new(); count];
    for (item, label) in labels.iter().enumerate() {
        groups[*label as usize].push(item as u32);
    }
    groups
}
