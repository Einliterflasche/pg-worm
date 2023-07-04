mod filter;
mod join;
mod table;
mod select;

use std::ops::Deref;

pub use filter::Filter;
pub use join::{Join, JoinType};
pub use table::{Column, TypedColumn};
pub use select::*;
use tokio_postgres::Row;

/// An executable query. Built using a [`QueryBuilder`].
pub struct Query;

impl Query {
    pub fn select<T: TryFrom<Row, Error = crate::Error>, const N: usize>(cols: [&'static dyn Deref<Target = Column>; N]) -> SelectBuilder<T> {
        let cols: Vec<_> = cols.into_iter().map(|i| i.deref()).collect();
        SelectBuilder::new(cols.as_slice())
    }
}
