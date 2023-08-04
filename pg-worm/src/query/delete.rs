use std::{
    future::{Future, IntoFuture},
    pin::Pin,
};

use super::{Executable, PushChunk, ToQuery, Where};

/// A struct for building `DELETE` queries.
pub struct Delete<'a> {
    table: &'static str,
    where_: Where<'a>,
}

impl<'a> ToQuery<'a, u64> for Delete<'a> {}

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
}

impl<'a> PushChunk<'a> for Delete<'a> {
    fn push_to_buffer<T>(&mut self, buffer: &mut super::Query<'a, T>) {
        buffer.0.push_str("DELETE FROM ");
        buffer.0.push_str(self.table);

        if !self.where_.is_empty() {
            buffer.0.push_str(" WHERE ");
            self.where_.push_to_buffer(buffer);
        }
    }
}

impl<'a> IntoFuture for Delete<'a> {
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;
    type Output = Result<u64, crate::Error>;

    fn into_future(mut self) -> Self::IntoFuture {
        let mut query = self.to_query();

        Box::pin(async move { query.exec().await })
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use pg_worm::prelude::*;

    #[derive(Model)]
    struct Book {
        id: i64,
        title: String,
    }

    #[test]
    fn delete_statement() {
        let q = Book::delete().to_query().0;
        assert_eq!(q, "DELETE FROM book");
    }

    #[test]
    fn delete_statement_with_where() {
        let q = Book::delete().where_(Book::id.eq(&4)).to_query().0;

        assert_eq!(q, "DELETE FROM book WHERE book.id = $1")
    }
}
