use tokio_postgres::types::ToSql;

use super::Query;

type Entry<'a> = (&'static str, &'a (dyn ToSql + Sync));

/// A struct for building insert queries
pub struct Insert<'a> {
    table: &'static str,
    entries: Vec<Entry<'a>>,
}

impl<'a> Insert<'a> {
    /// Start building a new `INSERT` query.
    pub fn new(table: &'static str) -> Self {
        Insert {
            table,
            entries: Vec::new(),
        }
    }

    /// Insert a value into a column
    pub fn entry(mut self, col: &'static str, val: &'a (dyn ToSql + Sync)) -> Self {
        self.entries.push((col, val));

        self
    }
}

impl<'a> From<Insert<'a>> for Query<'a, u64> {
    fn from(value: Insert<'a>) -> Self {
        let mut buffer = Query::default();

        buffer.0.push_str("INSERT INTO ");
        buffer.0.push_str(value.table);
        buffer.0.push_str(" (");

        value.entries.iter().for_each(|i| {
            buffer.0.push_str(i.0);
            buffer.0.push_str(", ");
        });

        buffer.0.pop();
        buffer.0.pop();
        buffer.0.push_str(" ) VALUES (");

        value.entries.iter().enumerate().for_each(|(i, (_, val))| {
            buffer.0.push_str(&format!("${}, ", i + 1));
            buffer.1.push(*val);
        });

        buffer.0.pop();
        buffer.0.pop();
        buffer.0.push(')');

        buffer
    }
}
