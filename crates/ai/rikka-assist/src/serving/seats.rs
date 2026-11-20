use rikka_tool::{Function, Tool};

#[derive(Debug, Default)]
pub(crate) struct Seats {
    next: u64,
    seats: Vec<SeatEntry>,
}

#[derive(Debug)]
pub(crate) struct SeatEntry {
    id: u64,
    title: String,
    functions: Vec<Function>,
}

impl SeatEntry {
    pub(crate) fn functions(&self) -> &[Function] {
        &self.functions
    }

    pub(crate) fn function(&self, tool: &str) -> Result<Function, String> {
        self.functions
            .iter()
            .find(|function| function.name() == tool)
            .cloned()
            .ok_or_else(|| format!("there is no tool named \"{tool}\" here"))
    }
}

impl Seats {
    pub(crate) fn add(&mut self, title: String, functions: Vec<Function>) -> u64 {
        self.next += 1;
        let id = self.next;
        self.seats.insert(
            0,
            SeatEntry {
                id,
                title,
                functions,
            },
        );
        id
    }

    pub(crate) fn remove(&mut self, id: u64) {
        self.seats.retain(|seat| seat.id != id);
    }

    pub(crate) fn retitle(&mut self, id: u64, title: String) {
        if let Some(seat) = self.seats.iter_mut().find(|seat| seat.id == id) {
            seat.title = title;
        }
    }

    pub(crate) fn used(&mut self, id: u64) {
        if let Some(at) = self.seats.iter().position(|seat| seat.id == id) {
            let seat = self.seats.remove(at);
            self.seats.insert(0, seat);
        }
    }

    pub(crate) fn first(&self) -> Option<&SeatEntry> {
        self.seats.first()
    }

    pub(crate) fn fronted(&self) -> Vec<Tool> {
        self.first().map_or_else(Vec::new, |seat| {
            seat.functions
                .iter()
                .map(|function| function.tool().clone())
                .collect()
        })
    }

    pub(crate) fn fronts(&self, tools: &[Tool]) -> bool {
        let fronted = self.first().map_or(&[][..], |seat| &seat.functions[..]);
        fronted.iter().map(Function::tool).eq(tools.iter())
    }

    pub(crate) fn select(&self, name: Option<&str>) -> Result<&SeatEntry, String> {
        let named = self.named();
        match name {
            None => named
                .first()
                .map(|(_, seat)| *seat)
                .ok_or_else(|| "no window is open to work in".to_owned()),
            Some(name) => named
                .iter()
                .find(|(called, _)| called == name)
                .map(|(_, seat)| *seat)
                .ok_or_else(|| {
                    format!(
                        "there is no window \"{name}\"; the windows are {}",
                        quoted(&named)
                    )
                }),
        }
    }

    pub(crate) fn listing(&self) -> String {
        let named = self.named();
        match named.is_empty() {
            true => "No window is open.".to_owned(),
            false => named
                .iter()
                .map(|(called, _)| format!("- {called}"))
                .collect::<Vec<String>>()
                .join("\n"),
        }
    }

    fn named(&self) -> Vec<(String, &SeatEntry)> {
        let mut named: Vec<(String, &SeatEntry)> = Vec::with_capacity(self.seats.len());
        for seat in &self.seats {
            let before = named
                .iter()
                .filter(|(_, earlier)| earlier.title == seat.title)
                .count();
            let called = match before {
                0 => seat.title.clone(),
                more => format!("{} ({})", seat.title, more + 1),
            };
            named.push((called, seat));
        }
        named
    }
}

fn quoted(named: &[(String, &SeatEntry)]) -> String {
    named
        .iter()
        .map(|(called, _)| format!("\"{called}\""))
        .collect::<Vec<String>>()
        .join(", ")
}
