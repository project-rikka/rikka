use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct State<L, A> {
    pub(crate) accept: A,
    pub(crate) edges: Vec<(L, usize)>,
}

impl<L, A: Default> State<L, A> {
    fn empty() -> Self {
        Self {
            accept: A::default(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Dawg<L, A> {
    pub(crate) states: Vec<State<L, A>>,
    pub(crate) root: usize,
}

impl<L, A> Dawg<L, A> {
    pub(crate) fn breadth_first(&self) -> Vec<usize> {
        let mut visited = vec![false; self.states.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::from([self.root]);
        visited[self.root] = true;
        while let Some(state) = queue.pop_front() {
            order.push(state);
            for &(_, target) in &self.states[state].edges {
                if !visited[target] {
                    visited[target] = true;
                    queue.push_back(target);
                }
            }
        }
        order
    }
}

struct Builder<L, A> {
    states: Vec<State<L, A>>,
    register: HashMap<State<L, A>, usize>,
    unchecked: Vec<(usize, usize)>,
    previous: Vec<L>,
}

impl<L: Copy + Eq + Hash, A: Clone + Default + Eq + Hash> Builder<L, A> {
    fn insert(&mut self, word: &[L], accept: A) {
        let common = word
            .iter()
            .zip(&self.previous)
            .take_while(|(one, other)| one == other)
            .count();
        self.minimize(common);
        let mut node = self.unchecked.last().map_or(0, |&(_, child)| child);
        for &symbol in &word[common..] {
            let child = self.states.len();
            self.states.push(State::empty());
            self.states[node].edges.push((symbol, child));
            self.unchecked.push((node, child));
            node = child;
        }
        self.states[node].accept = accept;
        self.previous.clear();
        self.previous.extend_from_slice(word);
    }

    fn minimize(&mut self, depth: usize) {
        while self.unchecked.len() > depth {
            let Some((parent, child)) = self.unchecked.pop() else {
                return;
            };
            match self.register.get(&self.states[child]) {
                Some(&equivalent) => {
                    if let Some(edge) = self.states[parent].edges.last_mut() {
                        edge.1 = equivalent;
                    }
                }
                None => {
                    self.register.insert(self.states[child].clone(), child);
                }
            }
        }
    }
}

pub(crate) fn build<'w, L, A>(sorted_words: impl IntoIterator<Item = (&'w [L], A)>) -> Dawg<L, A>
where
    L: Copy + Eq + Hash + 'w,
    A: Clone + Default + Eq + Hash,
{
    let mut builder = Builder {
        states: vec![State::empty()],
        register: HashMap::new(),
        unchecked: Vec::new(),
        previous: Vec::new(),
    };
    for (word, accept) in sorted_words {
        builder.insert(word, accept);
    }
    builder.minimize(0);
    Dawg {
        states: builder.states,
        root: 0,
    }
}
