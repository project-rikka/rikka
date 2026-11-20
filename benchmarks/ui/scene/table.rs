const ADJECTIVES: [&str; 25] = [
    "pretty",
    "large",
    "big",
    "small",
    "tall",
    "short",
    "long",
    "handsome",
    "plain",
    "quaint",
    "clean",
    "elegant",
    "easy",
    "angry",
    "crazy",
    "helpful",
    "mushy",
    "odd",
    "unsightly",
    "adorable",
    "important",
    "inexpensive",
    "cheap",
    "expensive",
    "fancy",
];

const COLOURS: [&str; 11] = [
    "red", "yellow", "blue", "green", "pink", "brown", "purple", "brown", "white", "black",
    "orange",
];

const NOUNS: [&str; 13] = [
    "table", "chair", "house", "bbq", "desk", "car", "pony", "cookie", "sandwich", "burger",
    "pizza", "mouse", "keyboard",
];

const SEED: u64 = 0x9e37_79b9_7f4a_7c15;

const UPDATED: &str = " !!!";

const SWAPPED: (usize, usize) = (1, 998);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: usize,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub rows: Vec<Row>,
    pub selected: Option<usize>,
    next_id: usize,
    random: u64,
}

impl Default for Table {
    fn default() -> Table {
        Table {
            rows: Vec::new(),
            selected: None,
            next_id: 1,
            random: SEED,
        }
    }
}

impl Table {
    #[must_use]
    pub fn with_rows(count: usize) -> Table {
        let mut table = Table::default();
        table.run(count);
        table
    }

    pub fn run(&mut self, count: usize) {
        self.rows = self.build(count);
        self.selected = None;
    }

    pub fn add(&mut self, count: usize) {
        let mut built = self.build(count);
        self.rows.append(&mut built);
    }

    pub fn update(&mut self) {
        for row in self.rows.iter_mut().step_by(10) {
            row.label.push_str(UPDATED);
        }
    }

    pub fn select(&mut self, id: usize) {
        self.selected = Some(id);
    }

    pub fn swap_rows(&mut self) {
        if self.rows.len() > SWAPPED.1 {
            self.rows.swap(SWAPPED.0, SWAPPED.1);
        }
    }

    pub fn remove(&mut self, id: usize) {
        self.rows.retain(|row| row.id != id);
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.selected = None;
    }

    #[must_use]
    pub fn is_selected(&self, row: &Row) -> bool {
        self.selected == Some(row.id)
    }

    fn build(&mut self, count: usize) -> Vec<Row> {
        (0..count)
            .map(|_| {
                let label = format!(
                    "{} {} {}",
                    ADJECTIVES[self.pick(ADJECTIVES.len())],
                    COLOURS[self.pick(COLOURS.len())],
                    NOUNS[self.pick(NOUNS.len())]
                );
                let id = self.next_id;
                self.next_id += 1;
                Row { id, label }
            })
            .collect()
    }

    fn pick(&mut self, bound: usize) -> usize {
        self.random ^= self.random >> 12;
        self.random ^= self.random << 25;
        self.random ^= self.random >> 27;
        (self.random.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % bound
    }
}
