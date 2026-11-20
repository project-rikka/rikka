use crate::question::Question;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionSet {
    id: u64,
    questions: Vec<Question>,
}

impl QuestionSet {
    #[must_use]
    pub fn new(id: u64, questions: Vec<Question>) -> QuestionSet {
        QuestionSet { id, questions }
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn questions(&self) -> &[Question] {
        &self.questions
    }
}
