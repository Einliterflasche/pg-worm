use std::{
    future::{Future, IntoFuture},
    marker::PhantomData,
    pin::Pin,
};

use tokio_postgres::types::ToSql;

use crate::TypedColumn;

use super::{
    push_all_with_sep, replace_question_marks, PushChunk, Query, QueryOutcome, SqlChunk, Where,
};

/// State representing that an UPDATE
/// has been set.
///
/// `UPDATE` queries in this state cannot be executed.
#[doc(hidden)]
pub struct NoneSet;
/// State representing that an UDPATE
/// has been set.
#[doc(hidden)]
pub struct SomeSet;

/// A struct for building `UPDATE` queries.
///
/// The query can only be executed once at least one
/// update has been made.
pub struct Update<'a, State = NoneSet> {
    table: &'static str,
    updates: Vec<SqlChunk<'a>>,
    where_: Where<'a>,
    state: PhantomData<State>,
}

impl<'a, T> Update<'a, T> {
    /// Begin building a new `UPDATE` query.
    pub fn new(table: &'static str) -> Update<'a, NoneSet> {
        Update {
            table,
            updates: vec![],
            where_: Where::Empty,
            state: PhantomData::<NoneSet>,
        }
    }

    /// Add a `WHERE` to the query.
    ///
    /// If called multiple times, the conditions are
    /// joined using `AND`.
    pub fn where_(mut self, where_: Where<'a>) -> Update<'a, T> {
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
    ) -> Update<'a, T> {
        let where_ = Where::new(statement.into(), params);

        self.where_(where_)
    }

    /// Add a `SET` instruction to your `UPDATE` query.
    ///
    /// This function has to be called at least once before
    /// you can execute the query.
    pub fn set<U: ToSql + Sync>(
        mut self,
        col: TypedColumn<U>,
        value: &'a U,
    ) -> Update<'a, SomeSet> {
        self.updates
            .push(SqlChunk(format!("{} = ?", col.column_name), vec![value]));

        Update {
            state: PhantomData::<SomeSet>,
            updates: self.updates,
            where_: self.where_,
            table: self.table,
        }
    }
}

impl<'a> From<Update<'a, SomeSet>> for Query<'a, u64> {
    fn from(mut from: Update<'a, SomeSet>) -> Self {
        let mut buffer = Query::default();

        // Which table to update
        buffer.0.push_str("UPDATE ");
        buffer.0.push_str(from.table);

        // Which updates to make
        buffer.0.push_str(" SET ");
        push_all_with_sep(&mut from.updates, &mut buffer, ", ");

        // Which rows to update
        if !from.where_.is_empty() {
            buffer.0.push_str(" WHERE ");
            from.where_.push_to_buffer(&mut buffer);
        }

        buffer.0 = replace_question_marks(buffer.0);

        buffer
    }
}

impl<'a> IntoFuture for Update<'a, SomeSet>
where
    Query<'a, u64>: From<Update<'a, SomeSet>>,
{
    type Output = Result<u64, crate::Error>;

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        let query = Query::from(self);

        Box::pin(async move { u64::exec(&query.0, query.1.as_slice()).await })
    }
}
