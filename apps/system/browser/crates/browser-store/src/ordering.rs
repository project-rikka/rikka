use std::collections::HashSet;

use rusqlite::{Connection, ParamsFromIter, params_from_iter};

use crate::error::Result;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Ordered {
    pub(crate) table: &'static str,
    pub(crate) scope: Option<(&'static str, i64)>,
}

impl Ordered {
    fn filter(self) -> String {
        match self.scope {
            Some((column, _)) => format!("WHERE {column} = ?1"),
            None => String::new(),
        }
    }

    fn arguments(self) -> ParamsFromIter<Option<i64>> {
        params_from_iter(self.scope.map(|(_, value)| value))
    }

    fn ids(self, connection: &Connection) -> Result<Vec<i64>> {
        let sql = format!(
            "SELECT id FROM {} {} ORDER BY position, id",
            self.table,
            self.filter()
        );
        let mut statement = connection.prepare_cached(&sql)?;
        let ids = statement.query_map(self.arguments(), |row| row.get(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn next_position(self, connection: &Connection) -> Result<i64> {
        let sql = format!(
            "SELECT coalesce(max(position) + 1, 0) FROM {} {}",
            self.table,
            self.filter()
        );
        let mut statement = connection.prepare_cached(&sql)?;
        let position = statement.query_row(self.arguments(), |row| row.get(0))?;
        Ok(position)
    }

    pub(crate) fn reorder(self, connection: &Connection, requested: &[i64]) -> Result<()> {
        let current = self.ids(connection)?;
        let known: HashSet<i64> = current.iter().copied().collect();
        let mut placed = HashSet::new();
        let mut order: Vec<i64> = requested
            .iter()
            .copied()
            .filter(|id| known.contains(id) && placed.insert(*id))
            .collect();
        order.extend(current.iter().copied().filter(|id| !placed.contains(id)));
        let sql = format!("UPDATE {} SET position = ?2 WHERE id = ?1", self.table);
        let mut update = connection.prepare_cached(&sql)?;
        for (position, id) in (0_i64..).zip(order) {
            update.execute([id, position])?;
        }
        Ok(())
    }
}
