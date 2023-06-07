mod filter;
mod join;
mod table;

pub use filter::Filter;
pub use table::{TypedColumn, Column};
pub use join::{Join, JoinType};

pub type DynCol = dyn Deref<Target = Column>;

use tokio_postgres::{types::ToSql, Row};

use std::ops::Deref;

use crate::_get_client;

pub struct Query {
    stmt: String,
    args: Vec<Box<dyn ToSql + Sync + Send>>
}

pub trait QueryBuilder {
    fn build(self) -> Query;
}

pub trait ToQuery {
    fn to_sql(self) -> String;
}

pub struct SelectBuilder {
    cols: Vec<&'static DynCol>,
    filter: Filter,
    joins: Vec<Join>,
    limit: Option<usize>
}

impl Query {
    /// Create a new query.
    fn new(stmt: String, args: Vec<Box<dyn ToSql + Send + Sync>>) -> Query {
        Query { 
            stmt, 
            args
        }
    }

    /// Start building a new SELECT query.
    /// 
    /// # Panics
    /// Panics if an empty array is provided.
    pub fn select<const N: usize>(cols: [&'static DynCol; N]) -> SelectBuilder {
        SelectBuilder::new(cols.into_iter().collect())
    }

    /// Get the query's statement
    pub fn stmt(&self) -> &str {
        &self.stmt
    }

    /// Execute a query.
    pub async fn exec(&self) -> Result<Vec<Row>, pg_worm::Error> {
        let client = _get_client()?;
        Ok(
            client.query(
            &self.stmt, 
            self.args
                .iter()
                .map(|i| &**i as _)
                .collect::<Vec<_>>()
                .as_slice()
            )
            .await?
        )
    }
}

impl SelectBuilder {
    /// Start building a new SELECT query.
    /// 
    /// # Panics
    /// Panics if an empty vec is provided.
    pub fn new(cols: Vec<&'static DynCol>) -> SelectBuilder {
        assert_ne!(cols.len(), 0, "must SELECT at least one column");

        SelectBuilder { 
            cols, 
            filter: Filter::all(), 
            joins: Vec::new(),
            limit: None
        }
    }

    /// Add a filter (WHERE clause) to the select query.
    /// 
    /// # Example
    /// 
    /// ```ignore
    /// let q = Query::select([&Book::title])
    ///     .filter(Book::id.eq(5))
    ///     .build();
    /// ```
    pub fn filter(mut self, new_filter: Filter) -> SelectBuilder {
        self.filter = self.filter & new_filter;

        self
    }

    /// Add a join to the select query.
    /// 
    /// # Example
    /// 
    /// ```ignore
    /// let q = Query::select([&Book::id, &Book::title, &Author::name])
    ///     .join(&Book::author, &Author::id, JoinType::Inner)
    ///     .filter(Author::name.eq("Marx"))
    ///     .build();
    /// ```
    pub fn join(mut self, column: &'static DynCol, on_column: &'static DynCol, join_type: JoinType) -> SelectBuilder {
        let join = Join::new(
            column,
            on_column,
            join_type
        );

        self.joins.push(join);

        self
    }

    /// Add a LIMIT to your query.
    pub fn limit(mut self, n: usize) -> SelectBuilder {
        self.limit = Some(n);

        self
    }
}

impl QueryBuilder for SelectBuilder {
    /// Build the query.
    fn build(self) -> Query {
        let select_cols = self.cols
            .iter()
            .map(|i| i.full_name())
            .collect::<Vec<_>>()
            .join(", ");

        let joins = self.joins
            .iter()
            .map(|i| i.to_sql())
            .collect::<Vec<String>>()
            .join(" ");

        let stmt = format!(
            "SELECT {select_cols} FROM {} {} {}",
            self.cols[0].table_name(),
            joins,
            self.filter.to_sql()
        );

        let args: Vec<Box<dyn ToSql + Sync + Send>> = self.filter
            .args();

        Query::new(stmt, args)
    }
}
