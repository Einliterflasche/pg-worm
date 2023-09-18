//! This module contains the logic for building queries,
//! as well as struct for representing columns.

mod delete;
mod select;
mod table;
mod transaction;
mod update;

pub use table::{Column, ColumnInfo, TypedColumn};

use std::{
    future::{Future, IntoFuture},
    marker::PhantomData,
    ops::{BitAnd, BitOr, Not},
    pin::Pin,
};

use async_trait::async_trait;
use tokio_postgres::{types::ToSql, Row, Transaction as PgTransaction};

use crate::{fetch_client, Client, Error, FromRow};

pub use delete::Delete;
pub use select::Select;
pub use transaction::*;
pub use update::{NoneSet, SomeSet, Update};

/// A trait implemented by everything that goes inside a query.
#[doc(hidden)]
pub trait PushChunk<'a> {
    /// Pushes the containing string and the params to the provided buffer.
    fn push_to_buffer<T>(&mut self, buffer: &mut Query<'a, T>);
}

/// A trait abstracting over `Client`s and `Transaction`s.
#[doc(hidden)]
#[async_trait]
pub trait Executor {
    /// Maps to tokio_postgres::Client::query.
    async fn query(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error>;
    /// Maps to tokio_postgres::Client::execute.
    async fn execute(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error>;
}

/// Trait used to mark exectuable queries. It is used
/// to make use of generics for executing them.
#[async_trait]
pub trait QueryOutcome: Sized {
    /// The actual function for executing a query.
    async fn exec(statement: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Self, crate::Error> {
        let client = fetch_client().await?;
        Self::exec_with(statement, params, &client).await
    }

    /// Execute the query given any viable `Executor`
    async fn exec_with(
        statement: &str,
        params: &[&(dyn ToSql + Sync)],
        client: impl Executor + Sync + Send,
    ) -> Result<Self, crate::Error>;
}

#[async_trait]
impl QueryOutcome for u64 {
    async fn exec_with(
        statement: &str,
        params: &[&(dyn ToSql + Sync)],
        client: impl Executor + Sync + Send,
    ) -> Result<u64, crate::Error> {
        client.execute(statement, params).await
    }
}

#[async_trait]
impl<T> QueryOutcome for Vec<T>
where
    T: FromRow,
{
    async fn exec_with(
        statement: &str,
        params: &[&(dyn ToSql + Sync)],
        client: impl Executor + Sync + Send,
    ) -> Result<Vec<T>, crate::Error> {
        let res = client.query(statement, params).await?;
        res.into_iter().map(T::try_from).collect()
    }
}

#[async_trait]
impl<T> QueryOutcome for Option<T>
where
    T: FromRow,
{
    async fn exec_with(
        statement: &str,
        params: &[&(dyn ToSql + Sync)],
        client: impl Executor + Sync + Send,
    ) -> Result<Option<T>, crate::Error> {
        let res = client.query(statement, params).await?;
        res.into_iter().map(T::try_from).next().transpose()
    }
}

/// A struct for storing a complete query along with
/// parameters and output type.
pub struct Query<'a, T = Vec<Row>>(pub String, Vec<&'a (dyn ToSql + Sync)>, PhantomData<T>);

/// A basic chunk of SQL and it's params.
///
/// This is bundes the params with the relevant part of the statement
/// and thus makes ordering them much easier.
#[doc(hidden)]
pub struct SqlChunk<'a>(pub String, pub Vec<&'a (dyn ToSql + Sync)>);

/// A generic implementation of `IntoFuture` for all viable query builders
/// ensures that each can be built _and_ executed simply
/// by calling `.await`.

/// Push multiple `PushChunk` objects to a buffer with a separator
/// between each of them.
///
/// Like `Vec::join()`.
fn push_all_with_sep<'a, T, U: PushChunk<'a>>(
    vec: &mut Vec<U>,
    buffer: &mut Query<'a, T>,
    sep: &str,
) {
    if vec.is_empty() {
        return;
    }

    for i in vec {
        i.push_to_buffer(buffer);
        buffer.0.push_str(sep);
    }

    // Remove the last `sep` as it's not
    // in between elements.
    buffer.0.truncate(buffer.0.len() - sep.len());
}

/// An enum representing the `WHERE` clause of a query.
pub enum Where<'a> {
    /// A number of conditions joined by `AND`.
    And(Vec<Where<'a>>),
    /// A number of conditions joined by `OR`.
    Or(Vec<Where<'a>>),
    /// A negated condition.
    Not(Box<Where<'a>>),
    /// A raw condition.
    Raw(SqlChunk<'a>),
    /// An empty `WHERE` clause.
    Empty,
}

/// Replace all `?` placeholders with the Postgres variant
/// `$1`, `$2`, etc.
fn replace_question_marks(stmt: String) -> String {
    // Since we change '?' to e.g. '$1' we need to
    // reserve some more space to avoid reallocating.
    const RESERVED: usize = 9;
    let mut buf = String::with_capacity(stmt.len() + RESERVED);

    // Tracking variable
    let mut last_index = 0;

    // Looping through all '?' in the string
    for (count, (i, _)) in stmt.match_indices('?').enumerate() {
        // Push everything until the '?'
        buf.push_str(&stmt[last_index..i]);

        // Push '$' including the number
        buf.push('$');
        buf.push_str(&(count + 1).to_string());

        last_index = i + 1;
    }

    // Push the tail
    buf.push_str(&stmt[last_index..]);

    buf
}

impl<'a, T> Default for Query<'a, T> {
    fn default() -> Self {
        Self("".into(), vec![], PhantomData::<T>)
    }
}

impl<'a, T> Query<'a, T> {
    /// Create a new query by passing a raw statement as well as parameters.
    pub fn new(stmt: String, params: Vec<&'a (dyn ToSql + Sync)>) -> Query<'a, T> {
        Query(replace_question_marks(stmt), params, PhantomData::<T>)
    }
}

impl<'a> PushChunk<'a> for SqlChunk<'a> {
    fn push_to_buffer<T>(&mut self, buffer: &mut Query<'a, T>) {
        buffer.0.push_str(&self.0);
        buffer.1.append(&mut self.1);
    }
}

#[async_trait]
impl<'a> Executor for &PgTransaction<'a> {
    async fn query(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        PgTransaction::query(self, stmt, params)
            .await
            .map_err(Error::from)
    }

    async fn execute(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        PgTransaction::execute(self, stmt, params)
            .await
            .map_err(Error::from)
    }
}

#[async_trait]
impl Executor for &Client {
    async fn query(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        (***self).query(stmt, params).await.map_err(Error::from)
    }

    async fn execute(&self, stmt: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        (***self).execute(stmt, params).await.map_err(Error::from)
    }
}

/// Implement IntoFuture for Query so that any executable Query
/// may be executed by calling `.await`.
impl<'a, T: Send + 'a> IntoFuture for Query<'a, T>
where
    T: QueryOutcome + Sync,
{
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'a>>;
    type Output = Result<T, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { T::exec(&self.0, self.1.as_slice()).await })
    }
}

impl<'a> Where<'a> {
    /// Create a new WHERE expression with parameters.
    pub(crate) fn new(expr: String, params: Vec<&'a (dyn ToSql + Sync)>) -> Where<'a> {
        Self::Raw(SqlChunk(expr, params))
    }

    /// Check whether the WHERE clause is empty.
    pub(crate) fn is_empty(&self) -> bool {
        use Where::*;

        match self {
            Empty => true,
            And(vec) => vec.iter().all(|i| i.is_empty()),
            Or(vec) => vec.iter().all(|i| i.is_empty()),
            Not(inner) => inner.is_empty(),
            Raw(chunk) => chunk.0.is_empty(),
        }
    }

    /// Combine two conditions using AND.
    ///
    /// You can also use the `&` operator.
    pub fn and(self, other: Where<'a>) -> Where<'a> {
        self.bitand(other)
    }

    /// Combine two conditions using OR.
    ///
    /// You can also use the `|` operator.
    ///
    /// # Example
    /// ```ignore
    /// Where::new("id = ?", vec![&7])
    ///     .or(Where::new("name = ?", vec![&"John"]))
    /// ```
    pub fn or(self, other: Where<'a>) -> Where<'a> {
        self.bitor(other)
    }
}

impl<'a> Default for Where<'a> {
    fn default() -> Self {
        Where::new("".into(), vec![])
    }
}

impl<'a> BitAnd for Where<'a> {
    type Output = Where<'a>;

    fn bitand(mut self, mut other: Self) -> Self::Output {
        use Where::*;

        if let Empty = self {
            return other;
        }

        if let Empty = other {
            return self;
        }

        // If self is already an AND variant,
        // simply add other to the vec.
        // This prevents unnecessary nesting.
        if let And(ref mut vec) = self {
            // If other is also AND append the whole vec.
            if let And(ref mut other_vec) = other {
                vec.append(other_vec);
            } else {
                vec.push(other);
            }
            return self;
        }

        if let And(ref mut vec) = other {
            vec.push(self);
            return other;
        }

        And(vec![self, other])
    }
}

impl<'a> BitOr for Where<'a> {
    type Output = Where<'a>;

    fn bitor(mut self, mut other: Self) -> Self::Output {
        use Where::*;

        if let Empty = self {
            return other;
        }
        if let Empty = other {
            return self;
        }

        // If self is already an OR variant,
        // simply add other to the vec.
        // This prevents unnecessary nesting.
        if let Or(ref mut vec) = self {
            // If other is also OR append the whole vec.
            if let And(ref mut other_vec) = other {
                vec.append(other_vec);
            } else {
                vec.push(other);
            }
            return self;
        }

        if let Or(ref mut vec) = other {
            vec.push(self);
            return other;
        }

        Or(vec![self, other])
    }
}

impl<'a> Not for Where<'a> {
    type Output = Where<'a>;

    fn not(self) -> Self::Output {
        use Where::*;

        if let Not(inner) = self {
            return *inner;
        }

        Not(Box::new(self))
    }
}

impl<'a> PushChunk<'a> for Where<'a> {
    fn push_to_buffer<T>(&mut self, buffer: &mut Query<'a, T>) {
        use Where::*;

        if self.is_empty() {
            return;
        }

        match self {
            Raw(chunk) => {
                chunk.push_to_buffer(buffer);
            }
            Not(inner) => {
                buffer.0.push_str("NOT (");
                inner.push_to_buffer(buffer);
                buffer.0.push(')');
            }
            And(vec) => {
                buffer.0.push('(');
                push_all_with_sep(vec, buffer, ") AND (");
                buffer.0.push(')');
            }
            Or(vec) => {
                buffer.0.push('(');
                push_all_with_sep(vec, buffer, ") OR (");
                buffer.0.push(')');
            }
            Empty => (),
        }
    }
}
