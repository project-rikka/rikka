use cut_project::Project;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Range {
    #[default]
    Whole,
    Marks,
}

impl Range {
    #[must_use]
    pub fn suggested(project: &Project) -> Range {
        let marks = project.marks();
        match marks.mark_in.is_some() || marks.mark_out.is_some() {
            true => Range::Marks,
            false => Range::Whole,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    first: i64,
    end: i64,
}

impl Span {
    #[must_use]
    pub fn new(first: i64, end: i64) -> Option<Span> {
        (first >= 0 && end > first).then_some(Span { first, end })
    }

    #[must_use]
    pub fn resolve(project: &Project, range: Range) -> Option<Span> {
        match range {
            Range::Whole => Span::new(0, project.end()),
            Range::Marks => {
                let marks = project.marks();
                Span::new(
                    marks.mark_in.unwrap_or(0),
                    marks.mark_out.map_or(project.end(), |out| out + 1),
                )
            }
        }
    }

    #[must_use]
    pub fn first(&self) -> i64 {
        self.first
    }

    #[must_use]
    pub fn end(&self) -> i64 {
        self.end
    }

    #[must_use]
    pub fn length(&self) -> i64 {
        self.end - self.first
    }
}
