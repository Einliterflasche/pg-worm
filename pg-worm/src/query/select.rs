use std::{ops::Deref, marker::PhantomData, future::{IntoFuture, Future}, pin::Pin};

use tokio_postgres::Row;

use crate::Column;
use super::{Where, Query, PushChunk, Executable};

/// A struct which holds the information needed to build
/// a `SELECT` query.
pub struct Select<'a, T = Vec<Row>> {
    cols: Vec<Column>,
    from: &'static str,
    where_: Where<'a>,
    marker: PhantomData<T>,
    limit: Option<u64>,
    offset: Option<u64>
}

impl<'a, T> Select<'a, T> {
    #[doc(hidden)]
    pub fn new(cols: &[&dyn Deref<Target = Column>], from: &'static str) -> Select<'a, T> {
        Select { 
            cols: cols.into_iter().map(|i| (**i).clone()).collect(), 
            from,
            where_: Where::Empty,
            marker: PhantomData::<T>,
            limit: None,
            offset: None
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

    /// Convert to a query.
    fn to_query(mut self) -> Query<'a, T> {
        let mut query = Query::default();
        self.push_to_buffer(&mut query);

        // Since we change '?' to e.g. '$1' we need to
        // reserve some more space to avoid reallocating the whole string.
        const RESERVED: usize = 9;
        let mut buf = String::with_capacity(query.0.len() + RESERVED);

        let mut counter = 1;
        let mut last_index = 0;

        for (i, _) in query.0.match_indices("?") {
            buf.push_str(&query.0[last_index..i]);
            buf.push('$');
            buf.push_str(&counter.to_string());
            
            counter += 1;
            last_index = i + 1;
        }
        // Push the tail
        buf.push_str(&query.0[last_index..]);
        // Update the string to the one with $-like placeholders
        query.0 = buf;

        query
    }
}

impl<'a, T> PushChunk<'a> for Select<'a, T> {
    fn push_to_buffer<B>(&mut self, buffer: &mut Query<'a, B>) {
        buffer.0.push_str("SELECT ");

        // Push the selected columns
        let cols = self.cols.iter()
            .map(|i| i.full_name())
            .collect::<Vec<_>>().join(", ");
        buffer.0.push_str(&cols);

        // Push the table from which the columns
        // are selected
        buffer.0.push_str(" FROM ");
        buffer.0.push_str(self.from);

        // If it exists, push the WHERE clause
        if !self.where_.is_empty() {
            buffer.0.push_str(" WHERE ");
            self.where_.push_to_buffer(buffer);
        }

        // If set, add a LIMIT
        if let Some(limit) = self.limit {
            buffer.0.push_str(" LIMIT ");
            buffer.0.push_str(&limit.to_string());
        }

        // If set, add an OFFSET
        if let Some(offset) = self.offset {
            buffer.0.push_str(" OFFSET ");
            buffer.0.push_str(&offset.to_string())
        }
    }
}

impl<'a, T> IntoFuture for Select<'a, T>
where 
    Query<'a, T>: Executable,
    T: 'a
{
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;
    type Output = Result<<Query<'a, T> as Executable>::Output, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        let query = self.to_query();

        Box::pin(async move {
            query.exec().await
        })
    }
}

#[cfg(test)] 
mod test {
    use crate::prelude::*;

    #[derive(Model)]
    struct Book {
        #[column(primary_key, auto)]
        id: i64,
        title: String
    }

    #[test]
    fn select_limit() {
        let query = Book::select()
            .limit(3).to_query().0;
        assert_eq!(query, "SELECT book.id, book.title FROM book LIMIT 3");
    }

    #[test]
    fn select_offset() {
        let query = Book::select()
            .offset(4).to_query().0;
        assert_eq!(query, "SELECT book.id, book.title FROM book OFFSET 4");
    }
}