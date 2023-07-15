pub mod select;
pub mod table;

pub use table::{Column, TypedColumn};

use std::{ops::{BitAnd, BitOr, Not, Deref}, marker::PhantomData};

use async_trait::async_trait;
use tokio_postgres::{types::ToSql, Row};

use crate::{_get_client, Model};

pub use select::Select;

pub trait PushChunk<'a> {
    fn push_to_buffer<T>(&mut self, buffer: &mut Query<'a, T>);
}

#[async_trait]
pub trait Executable {
    /// What output should this query result in?
    type Output;

    async fn exec(self) -> Result<Self::Output, crate::Error>;
}

pub struct Query<'a, T = Vec<Row>>(
    pub String, 
    Vec<&'a (dyn ToSql + Sync)>, 
    PhantomData<T>
);
pub struct SqlChunk<'a>(pub String, pub Vec<&'a (dyn ToSql + Sync)>);

pub enum Where<'a> {
    And(Vec<Where<'a>>),
    Or(Vec<Where<'a>>),
    Not(Box<Where<'a>>),
    Raw(SqlChunk<'a>),
    Empty
}

pub fn select<'a, T: Model<T>>(cols: &[&dyn Deref<Target = Column>]) -> Select<'a, Vec<T>> {
    Select::new(cols).from(T::table_name())
}

impl<'a, T> Default for Query<'a, T> {
    fn default() -> Self {
        Self("".into(), vec![], PhantomData::<T>)
    }
}

#[async_trait]
impl<'a, T> Executable for Query<'a, Vec<T>>
where
    T: TryFrom<Row, Error = crate::Error> + Send
{
    type Output = Vec<T>;

    async fn exec(self) -> Result<Self::Output, crate::Error> {
        let client = _get_client()?;
        let rows = client.query(&self.0, &self.1).await?;

        rows
            .into_iter()
            .map(|i| T::try_from(i))
            .collect()
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
            And(vec) => vec.iter()
                .all(|i| i.is_empty()),
            Or(vec) => vec.iter()
                .all(|i| i.is_empty()),
            Not(inner) => inner.is_empty(),
            Raw(chunk) => chunk.0.is_empty()
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

    /// Push multiple `Where` objects to a buffer with a separator
    /// between each of them. 
    /// 
    /// Like `Vec::join()`.
    fn push_all_with_sep<T>(vec: &mut Vec<Where<'a>>, buffer: &mut Query<'a, T>, sep: &str) {
        for i in vec {
            i.push_to_buffer(buffer);
            buffer.0.push_str(sep);
        }

        // Remove the last `sep` as it's not
        // in between elements.
        buffer.0.truncate(
            buffer.0.len() - sep.len()
        );
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
            return self
        }

        if let And(ref mut vec) = other {
            vec.push(self);
            return other
        }

        And(vec![self, other])
    }
}

impl<'a> BitOr for Where<'a> {
    type Output = Where<'a>;

    fn bitor(mut self, mut other: Self) -> Self::Output {
        use Where::*;

        if let Empty = self {
            return other
        }
        if let Empty = other {
            return self
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
            return self
        }

        if let Or(ref mut vec) = other {
            vec.push(self);
            return other
        }

        Or(vec![self, other])
    }
}

impl<'a> Not for Where<'a> {
    type Output = Where<'a>;

    fn not(self) -> Self::Output {
        use Where::*;

        if let Not(inner) = self {
            return *inner
        }

        Not(Box::new(self))
    }
}

impl<'a> PushChunk<'a> for Where<'a> {
    fn push_to_buffer<T>(&mut self, buffer: &mut Query<'a, T>) {
        use Where::*;

        if self.is_empty() {
            return
        }

        match self {
            Raw(chunk) => {
                buffer.0.push_str(&chunk.0);
                buffer.1.append(&mut chunk.1);
            },
            Not(inner) => {
                buffer.0.push_str("NOT (");
                inner.push_to_buffer(buffer);
                buffer.0.push(')');
            },
            And(vec) => {
                buffer.0.push('(');
                Where::push_all_with_sep(vec, buffer, ") AND (");
                buffer.0.push(')');
            },
            Or(vec) => {
                buffer.0.push('(');
                Where::push_all_with_sep(vec, buffer, ") OR (");
                buffer.0.push(')');
            },
            Empty => ()
        }
    }
}
