mod filter;
mod join;
mod table;

pub use filter::Filter;
pub use table::{Column, ColumnShared};

pub use join::{Join, JoinType};

use std::ops::Deref;

type DynCol = dyn Deref<Target = ColumnShared>;

pub struct Query {
    cols: Vec<ColumnShared>,
    query_type: QueryType,
    filter: Filter,
    joins: Vec<Join>,
}

enum QueryType {
    Insert,
    Select,
    Update,
    Delete,
}

/// Implement a function which creates a new join of the
/// specified type.
macro_rules! impl_join {
    ($id:ident, $ty:expr) => {
        pub fn $id(mut self, c1: &'static DynCol, c2: &'static DynCol) -> Query {
            self.joins.push(Join::new(c1, c2, $ty));

            self
        }
    };
}

impl Query {
    pub fn select<const N: usize>(cols: [&DynCol; N]) -> Query {
        let cols: Vec<ColumnShared> = cols.into_iter().map(|i| **i).collect();

        Query {
            query_type: QueryType::Select,
            filter: Filter::all(),
            cols,
            joins: Vec::new(),
        }
    }

    /// Add a filter to your query.
    ///
    /// If there already is one, they are joined using `AND`.   
    pub fn filter(mut self, filter: Filter) -> Query {
        self.filter = self.filter & filter;

        self
    }

    impl_join!(inner_join, JoinType::Inner);
    impl_join!(outer_join, JoinType::Outer);
    impl_join!(right_join, JoinType::Right);
    impl_join!(left_join, JoinType::Left);

    pub fn to_sql(&self) -> String {
        let query_type = match self.query_type {
            QueryType::Select => "SELECT",
            QueryType::Insert => "INSERT",
            QueryType::Update => "UPDATE",
            QueryType::Delete => "DELETE"
        };

        format!("{query_type} ...")
    }
}
