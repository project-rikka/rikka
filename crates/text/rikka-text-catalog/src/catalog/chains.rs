use rikka_sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::families::FamilyId;
use crate::matching::ChainId;

#[derive(Debug, Default)]
pub(crate) struct Chains {
    inner: RwLock<Interned>,
}

#[derive(Debug, Default)]
struct Interned {
    chains: Vec<Arc<[FamilyId]>>,
    ids: HashMap<Arc<[FamilyId]>, ChainId>,
}

impl Chains {
    pub(crate) fn intern(&self, chain: Vec<FamilyId>) -> ChainId {
        let mut interned = self.inner.write();
        if let Some(&id) = interned.ids.get(chain.as_slice()) {
            return id;
        }
        let chain: Arc<[FamilyId]> = chain.into();
        let id = ChainId(interned.chains.len() as u32);
        interned.chains.push(chain.clone());
        interned.ids.insert(chain, id);
        id
    }

    pub(crate) fn get(&self, id: ChainId) -> Arc<[FamilyId]> {
        self.inner
            .read()
            .chains
            .get(id.0 as usize)
            .cloned()
            .unwrap_or_else(|| Arc::from([]))
    }
}
