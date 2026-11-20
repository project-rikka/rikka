use rikka_thread::{Question, QuestionSet, Selection};
use rikka_ui::TextDocument;

#[derive(Debug)]
pub struct QuestionForm {
    survey: QuestionSet,
    at: usize,
    picks: Vec<Pick>,
}

#[derive(Debug, Default)]
pub(crate) struct Pick {
    selected: Vec<usize>,
    wrote: TextDocument,
}

impl QuestionForm {
    #[must_use]
    pub fn new(survey: QuestionSet) -> Option<QuestionForm> {
        if survey.questions().is_empty() {
            return None;
        }
        let picks = survey.questions().iter().map(|_| Pick::default()).collect();
        Some(QuestionForm {
            survey,
            at: 0,
            picks,
        })
    }

    pub fn id(&self) -> u64 {
        self.survey.id()
    }

    pub(crate) fn question(&self) -> &Question {
        &self.survey.questions()[self.at]
    }

    pub(crate) fn at(&self) -> usize {
        self.at
    }

    pub(crate) fn count(&self) -> usize {
        self.survey.questions().len()
    }

    pub(crate) fn pick(&self) -> &Pick {
        &self.picks[self.at]
    }

    pub fn choose(&mut self, option: usize) {
        if option >= self.question().options().len() {
            return;
        }

        let multi = self.question().is_multi_select();
        let pick = &mut self.picks[self.at];
        match multi {
            true => match pick
                .selected
                .iter()
                .position(|selected_option| *selected_option == option)
            {
                Some(at) => {
                    pick.selected.remove(at);
                }
                None => {
                    pick.selected.push(option);
                    pick.selected.sort_unstable();
                }
            },
            false => {
                pick.selected = vec![option];
                pick.wrote.set_text("");
            }
        }
    }

    pub fn wrote(&mut self) {
        let multi = self.question().is_multi_select();
        let pick = &mut self.picks[self.at];
        if !multi && !pick.wrote.text().trim().is_empty() {
            pick.selected.clear();
        }
    }

    pub fn back(&mut self) {
        self.at = self.at.saturating_sub(1);
    }

    pub fn ahead(&mut self) -> bool {
        if self.is_last() {
            return false;
        }
        self.at += 1;
        true
    }

    pub(crate) fn is_first(&self) -> bool {
        self.at == 0
    }

    pub(crate) fn is_last(&self) -> bool {
        self.at + 1 >= self.count()
    }

    pub(crate) fn is_answered(&self) -> bool {
        self.picks.iter().any(|pick| !pick.is_empty())
    }

    pub fn into_answers(self) -> Vec<Selection> {
        let QuestionForm { survey, picks, .. } = self;
        survey
            .questions()
            .iter()
            .zip(picks)
            .map(|(question, pick)| pick.settle(question))
            .collect()
    }
}

impl Pick {
    pub(crate) fn is_chosen(&self, option: usize) -> bool {
        self.selected.contains(&option)
    }

    pub(crate) fn wrote(&self) -> &TextDocument {
        &self.wrote
    }

    fn is_empty(&self) -> bool {
        self.selected.is_empty() && self.wrote.text().trim().is_empty()
    }

    fn settle(self, question: &Question) -> Selection {
        let selections = self
            .selected
            .iter()
            .filter_map(|at| question.options().get(*at))
            .map(|option| option.label().to_owned())
            .collect();
        Selection::new(selections, Some(self.wrote.text().trim().to_owned()))
    }
}
