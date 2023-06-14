mod filter;
mod join;
mod table;

pub use filter::Filter;
pub use join::{Join, JoinType};
pub use table::{Column, TypedColumn};

pub type DynCol = dyn Deref<Target = Column>;

use tokio_postgres::{types::ToSql, Row};

use std::{marker::PhantomData, ops::Deref};

use crate::_get_client;

/// An executable query. Built using a [`QueryBuilder`].
pub struct Query<T = Rows> {
    stmt: String,
    args: Vec<Box<dyn ToSql + Sync + Send>>,
    method: PhantomData<T>,
}

/// Implements either all or the specified traits
/// for a given type.
macro_rules! impl_for {
    ($ty:ty, $($trait:ty),+) => {
        $(
            impl $trait for $ty { }
        )*
    };

    ($ty:ty) => {
        impl_for!(
            $ty,
            Filterable,
            Joinable,
            Limitable
        );
    }
}

pub trait Filterable {}
pub trait Joinable {}
pub trait Limitable {}

pub struct Select;
pub struct Insert;
pub struct Delete;
pub struct Update;

pub struct Rows;
pub struct RowsAffected;

impl_for!(Select);
impl_for!(Delete);
impl_for!(Update);

pub struct QueryBuilder<Type> {
    cols: Vec<&'static DynCol>,
    filter: Filter,
    joins: Vec<Join>,
    limit: Option<usize>,
    _type: PhantomData<Type>,
}

/// This trait is implemented by anything
/// that goes into a query.
pub trait ToQuery {
    fn to_sql(&self) -> String;
}

impl<T> Query<T> {
    /// Create a new query.
    fn new(stmt: String, args: Vec<Box<dyn ToSql + Send + Sync>>) -> Query<T> {
        Query {
            stmt,
            args,
            method: PhantomData::<T>,
        }
    }

    /// Get the query's statement
    pub fn stmt(&self) -> &str {
        &self.stmt
    }
}

impl Query<Rows> {
    /// Execute a query and return the resulting rows.
    pub async fn exec(&self) -> Result<Vec<Row>, pg_worm::Error> {
        let client = _get_client()?;
        let rows = client
            .query(
                &self.stmt,
                self.args
                    .iter()
                    .map(|i| &**i as _)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .await?;

        Ok(rows)
    }
}

impl Query<RowsAffected> {
    pub async fn exec(&self) -> Result<u64, pg_worm::Error> {
        let client = _get_client()?;
        let res = client
            .execute(
                &self.stmt,
                self.args
                    .iter()
                    .map(|i| &**i as _)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .await?;

        Ok(res)
    }
}

impl<T: Filterable> QueryBuilder<T> {
    /// Add a [`Filter`] to the query.
    ///
    /// # Example
    ///
    /// ```
    /// use pg_worm::prelude::*;
    ///
    /// #[derive(Model)]
    /// struct Book {
    ///     id: i64,
    ///     title: String
    /// }
    ///
    /// let q = QueryBuilder::<Select>::new([&Book::title])
    ///     .filter(Book::id.eq(5))
    ///     .build();
    /// ```
    pub fn filter(mut self, new_filter: Filter) -> QueryBuilder<T> {
        self.filter = self.filter & new_filter;

        self
    }
}

impl<T: Joinable> QueryBuilder<T> {
    /// Add a [`Join`] to the query.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pg_worm::{Model, Query, QueryBuilder, JoinType};
    ///
    /// #[derive(Model)]
    /// struct Book {
    ///     #[column(primary_key, auto)]
    ///     id: i64,
    ///     title: String,
    ///     author_id: i64
    /// }
    ///
    /// #[derive(Model)]
    /// struct Author {
    ///     #[column(primary_key, auto)]
    ///     id: i64,
    ///     name: String
    /// }
    ///
    /// let q = Query::select([&Book::id, &Book::title, &Author::name])
    ///     .join(&Book::author_id, &Author::id, JoinType::Inner)
    ///     .filter(Author::name.eq("Marx"))
    ///     .build();
    /// ```
    pub fn join(
        mut self,
        column: &'static DynCol,
        on_column: &'static DynCol,
        join_type: JoinType,
    ) -> QueryBuilder<T> {
        let join = Join::new(column, on_column, join_type);

        self.joins.push(join);

        self
    }
}

impl<T: Limitable> QueryBuilder<T> {
    /// Add a LIMIT to your query.
    pub fn limit(mut self, n: usize) -> QueryBuilder<T> {
        self.limit = Some(n);

        self
    }
}

impl QueryBuilder<Select> {
    /// Start building a SELECT query.
    /// `cols` must not be empty.
    ///
    /// # Panics
    /// Panics if `cols.len() == 0`.
    pub fn new<const N: usize>(cols: [&'static DynCol; N]) -> QueryBuilder<Select> {
        assert!(!cols.is_empty(), "must select something");

        QueryBuilder {
            cols: cols.into_iter().collect(),
            filter: Filter::all(),
            joins: Vec::new(),
            limit: None,
            _type: PhantomData::<Select>,
        }
    }

    /// Build the query.
    pub fn build(self) -> Query {
        let select_cols = self
            .cols
            .iter()
            .map(|i| i.full_name())
            .collect::<Vec<_>>()
            .join(", ");

        let stmt = format!(
            "SELECT {select_cols} FROM {} {} {} {}",
            self.cols[0].table_name(),
            self.joins.to_sql(),
            self.filter.to_sql(),
            self.limit.to_sql()
        );

        Query::<Rows>::new(stmt, self.filter.args())
    }
}

impl QueryBuilder<Delete> {
    pub fn new<const N: usize>(cols: [&'static DynCol; N]) -> QueryBuilder<Delete> {
        assert!(!cols.is_empty(), "must delete from somewhere");

        QueryBuilder {
            cols: cols.into_iter().collect(),
            filter: Filter::all(),
            joins: Vec::new(),
            limit: None,
            _type: PhantomData::<Delete>,
        }
    }

    pub fn build(self) -> Query<RowsAffected> {
        let delete_table = self.cols[0].table_name();

        let stmt = format!(
            "DELETE FROM {delete_table} {} {} {}",
            self.joins.to_sql(),
            self.filter.to_sql(),
            self.limit.to_sql()
        );

        Query::<RowsAffected>::new(stmt, self.filter.args())
    }
}

impl<T: ToQuery> ToQuery for Option<T> {
    fn to_sql(&self) -> String {
        if let Some(x) = self {
            format!("LIMIT {}", x.to_sql())
        } else {
            String::new()
        }
    }
}

impl ToQuery for usize {
    fn to_sql(&self) -> String {
        self.to_string()
    }
}

impl ToQuery for Vec<Join> {
    fn to_sql(&self) -> String {
        self.iter()
            .map(|i| i.to_sql())
            .collect::<Vec<_>>()
            .join(" ")
    }
}
