use std::{
    future::{Future, IntoFuture},
    marker::PhantomData,
    ops::Deref,
    pin::Pin,
};

use tokio_postgres::{types::ToSql, Row};

use super::{replace_question_marks, PushChunk, Query, QueryOutcome, Where};
use crate::Column;

/// A struct which holds the information needed to build
/// a `SELECT` query.
pub struct Select<'a, T = Vec<Row>> {
    cols: Vec<Column>,
    from: &'static str,
    where_: Where<'a>,
    marker: PhantomData<T>,
    limit: Option<u64>,
    offset: Option<u64>,
}

impl<'a, T> Select<'a, T> {
    #[doc(hidden)]
    pub fn new(cols: &[&dyn Deref<Target = Column>], from: &'static str) -> Select<'a, T> {
        Select {
            cols: cols.iter().map(|i| (***i)).collect(),
            from,
            where_: Where::Empty,
            marker: PhantomData::<T>,
            limit: None,
            offset: None,
        }
    }

    /// Add a `WHERE` clause to your query.
    ///
    /// If used multiple time, the conditions are joined
    /// using `AND`.
    pub fn where_(mut self, where_: Where<'a>) -> Select<'a, T> {
        if self.where_.is_empty() {
            self.where_ = where_;
        } else {
            self.where_ = self.where_.and(where_);
        }

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
    ) -> Select<'a, T> {
        let where_ = Where::new(statement.into(), params);

        self.where_(where_)
    }

    /// Add a `LIMIT` to your query.
    pub fn limit(mut self, limit: u64) -> Select<'a, T> {
        self.limit = Some(limit);

        self
    }

    /// Add an `OFFSET` to your query.
    pub fn offset(mut self, offset: u64) -> Select<'a, T> {
        self.offset = Some(offset);

        self
    }
}

impl<'a, T> From<Select<'a, T>> for Query<'a, T> {
    fn from(mut from: Select<'a, T>) -> Self {
        let mut buffer = Query::default();

        buffer.0.push_str("SELECT ");

        // Push the selected columns
        let cols = from
            .cols
            .iter()
            .map(|i| i.full_name())
            .collect::<Vec<_>>()
            .join(", ");
        buffer.0.push_str(&cols);

        // Push the table from which the columns
        // are selected
        buffer.0.push_str(" FROM ");
        buffer.0.push_str(from.from);

        // If it exists, push the WHERE clause
        if !from.where_.is_empty() {
            buffer.0.push_str(" WHERE ");
            from.where_.push_to_buffer(&mut buffer);
        }

        // If set, add a LIMIT
        if let Some(limit) = from.limit {
            buffer.0.push_str(" LIMIT ");
            buffer.0.push_str(&limit.to_string());
        }

        // If set, add an OFFSET
        if let Some(offset) = from.offset {
            buffer.0.push_str(" OFFSET ");
            buffer.0.push_str(&offset.to_string())
        }

        buffer.0 = replace_question_marks(buffer.0);

        buffer
    }
}

impl<'a, T: Sync + Send + 'a> IntoFuture for Select<'a, T>
where
    T: QueryOutcome,
    Query<'a, T>: From<Select<'a, T>>,
{
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;
    type Output = Result<T, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        let query: Query<'_, T> = self.into();
        Box::pin(async move { T::exec(&query.0, query.1.as_slice()).await })
    }
}

#[cfg(test)]
mod test {
    #![allow(dead_code)]
    use crate::prelude::*;
    use crate::query::Query;

    #[derive(Model)]
    struct Book {
        #[column(primary_key, auto)]
        id: i64,
        title: String,
    }

    #[test]
    fn select_limit() {
        let query: Query<'_, Vec<Book>> = Book::select().limit(3).into();
        assert_eq!(query.0, "SELECT book.id, book.title FROM book LIMIT 3");
    }

    #[test]
    fn select_offset() {
        let query: Query<'_, Vec<Book>> = Book::select().offset(4).into();
        assert_eq!(query.0, "SELECT book.id, book.title FROM book OFFSET 4");
    }
}
