use std::{
    future::{Future, IntoFuture},
    pin::Pin,
};

use tokio_postgres::types::ToSql;

use super::{replace_question_marks, PushChunk, Query, QueryOutcome, Where};

/// A struct for building `DELETE` queries.
pub struct Delete<'a> {
    table: &'static str,
    where_: Where<'a>,
}

impl<'a> Delete<'a> {
    /// Start building a new `DELETE` query.
    pub fn new(table: &'static str) -> Delete<'a> {
        Delete {
            table,
            where_: Where::Empty,
        }
    }

    /// Add a `WHERE` clause to your `DELETE` query.
    ///
    /// If called multiple times, the conditions are joined using `AND`.
    pub fn where_(mut self, where_: Where<'a>) -> Delete<'a> {
        self.where_ = self.where_.and(where_);

        self
    }

    /// Add a raw `WHERE` clause to your query.
    ///
    /// You can reference the `params` by using the `?` placeholder in your statement.
    ///
    /// Note: you need to pass the exact types Postgres is expecting.
    /// Failure to do so will result in (sometimes confusing) runtime errors.
    ///
    /// Otherwise this behaves exactly like `where_`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// Book::select()
    ///     .where_(Book::id.neq(&3))
    ///     .where_raw("complex_function(book.title, ?, ?)", vec![&true, &"Foobar"])
    ///     .await?;
    /// ```
    pub fn where_raw(
        self,
        statement: impl Into<String>,
        params: Vec<&'a (dyn ToSql + Sync)>,
    ) -> Delete<'a> {
        let where_ = Where::new(statement.into(), params);

        self.where_(where_)
    }
}

impl<'a> From<Delete<'a>> for Query<'a, u64> {
    fn from(mut delete: Delete<'a>) -> Query<'a, u64> {
        let mut buffer = Query::default();
        buffer.0.push_str("DELETE FROM ");
        buffer.0.push_str(delete.table);

        if !delete.where_.is_empty() {
            buffer.0.push_str(" WHERE ");
            delete.where_.push_to_buffer(&mut buffer);
        }

        buffer.0 = replace_question_marks(buffer.0);

        buffer
    }
}

impl<'a> IntoFuture for Delete<'a> {
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;
    type Output = Result<u64, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        let query = Query::from(self);

        Box::pin(async move { u64::exec(&query.0, query.1.as_slice()).await })
    }
}
