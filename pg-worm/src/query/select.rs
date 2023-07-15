use std::{ops::Deref, marker::PhantomData, future::{IntoFuture, Future}, pin::Pin};

use crate::Column;
use super::{Where, Query, PushChunk, Executable};

pub struct Select<'a, T> {
    cols: Vec<Column>,
    from: &'static str,
    where_: Where<'a>,
    marker: PhantomData<T>
}

impl<'a, T> Select<'a, T> {
    pub(crate) fn new(cols: &[&dyn Deref<Target = Column>]) -> Select<'a, T> {
        Select { 
            cols: cols.into_iter().map(|i| i.deref().deref().clone()).collect(), 
            from: "", 
            where_: Where::Empty,
            marker: PhantomData::<T>
        }
    }

    /// Set the table `FROM` which the columns should be selected.
    pub fn from(mut self, from: &'static str) -> Select<'a, T> {
        self.from = from;

        self
    }

    /// Add a WHERE clause to your query. 
    /// 
    /// If used multiple time, the conditions are joined 
    /// using `AND`.
    pub fn where_(mut self, where_: Where<'a>) -> Select<'a, T> {
        self.where_ = self.where_.and(where_);

        self
    }

    /// Convert to a query.
    pub fn to_query(mut self) -> Query<'a, T> {
        let mut query = Query::default();
        self.push_to_buffer(&mut query);

        // Since we change '?' to e.g. '$1' we need to
        // reserve some more space to avoid reallocating the whole string.
        const RESERVED: usize = 9;
        let mut buf = String::with_capacity(query.0.len() + RESERVED);

        let mut counter = 1;
        let mut last_index = 0;

        for (i, _) in query.0.match_indices("?") {
            buf.push_str(&query.0[0..i]);
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
            .map(|i| i.column_name())
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
