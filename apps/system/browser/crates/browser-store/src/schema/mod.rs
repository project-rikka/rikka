mod declared;
mod objects;

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::error::Result;
use declared::{Declared, Kind};
use objects::OBJECTS;

const INTERNAL_PREFIX: &str = "sqlite_";
const VIRTUAL: &str = "CREATE VIRTUAL TABLE";

struct Stored {
    kind: Kind,
    sql: Option<String>,
}

impl Stored {
    fn is_virtual(&self) -> bool {
        self.sql
            .as_deref()
            .is_some_and(|sql| sql.starts_with(VIRTUAL))
    }
}

pub(crate) fn prepare(connection: &mut Connection) -> Result<()> {
    let declared: Vec<Declared> = OBJECTS.iter().copied().map(Declared::parse).collect();
    connection.pragma_update(None, "foreign_keys", false)?;
    let converged = converge(connection, &declared);
    connection.pragma_update(None, "foreign_keys", true)?;
    converged
}

fn converge(connection: &mut Connection, declared: &[Declared]) -> Result<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let stored = stored(&transaction)?;
    let stale = stale_tables(declared, &stored);
    for object in declared.iter().filter(|object| stale.contains(object.name)) {
        tracing::warn!(
            table = object.name,
            "browser store table does not match its declaration, recreating it empty"
        );
        drop_object(&transaction, Kind::Table, object.name)?;
    }
    let mut undeclared: Vec<(&String, &Stored)> = stored
        .iter()
        .filter(|(name, _)| !stale.contains(name.as_str()))
        .filter(|(name, object)| {
            !declared
                .iter()
                .find(|declared| declared.name == name.as_str())
                .is_some_and(|declared| {
                    declared.kind == object.kind
                        && object
                            .sql
                            .as_deref()
                            .is_some_and(|sql| declared.matches(sql))
                })
        })
        .collect();
    undeclared.sort_by_key(|(_, object)| !object.is_virtual());
    for (name, object) in undeclared {
        tracing::info!(name = %name, "dropping undeclared browser store object");
        drop_object(&transaction, object.kind, name)?;
    }
    let present = stored_names(&transaction)?;
    for object in declared
        .iter()
        .filter(|object| !present.contains(object.name))
    {
        transaction.execute_batch(object.sql)?;
    }
    transaction.commit()?;
    Ok(())
}

fn stored(transaction: &Transaction<'_>) -> Result<HashMap<String, Stored>> {
    let mut statement = transaction.prepare("SELECT type, name, sql FROM sqlite_schema")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut stored = HashMap::new();
    for row in rows {
        let (kind, name, sql) = row?;
        if name.starts_with(INTERNAL_PREFIX) {
            continue;
        }
        if let Some(kind) = Kind::from_type(&kind) {
            stored.insert(name, Stored { kind, sql });
        }
    }
    Ok(stored)
}

fn stored_names(transaction: &Transaction<'_>) -> Result<HashSet<String>> {
    let mut statement = transaction.prepare("SELECT name FROM sqlite_schema")?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

fn stale_tables<'a>(
    declared: &'a [Declared],
    stored: &HashMap<String, Stored>,
) -> HashSet<&'a str> {
    let mut stale: HashSet<&str> = declared
        .iter()
        .filter(|object| object.kind == Kind::Table)
        .filter(|object| {
            stored.get(object.name).is_some_and(|stored| {
                stored.kind != Kind::Table
                    || !stored.sql.as_deref().is_some_and(|sql| object.matches(sql))
            })
        })
        .map(|object| object.name)
        .collect();
    loop {
        let dependents: Vec<&str> = declared
            .iter()
            .filter(|object| object.kind == Kind::Table && !stale.contains(object.name))
            .filter(|object| object.depends.iter().any(|table| stale.contains(table)))
            .filter(|object| stored.contains_key(object.name))
            .map(|object| object.name)
            .collect();
        if dependents.is_empty() {
            return stale;
        }
        stale.extend(dependents);
    }
}

fn drop_object(transaction: &Transaction<'_>, kind: Kind, name: &str) -> Result<()> {
    transaction.execute_batch(&format!(
        "DROP {} IF EXISTS \"{}\"",
        kind.keyword(),
        name.replace('"', "\"\"")
    ))?;
    Ok(())
}
