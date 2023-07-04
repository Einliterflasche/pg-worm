use std::{marker::PhantomData, task::{Poll, Context}, pin::{Pin, pin}, ops::DerefMut, sync::Mutex};

use futures::{Future, FutureExt, pin_mut};
use tokio_postgres::{types::ToSql, Row};

use crate::{_get_client, Column, Filter, Error};

pub struct SelectBuilder<T> {
    cols: String,
    table: String,
    filter: Filter,
    parse_to: PhantomData<T>,
    fut: Mutex<Option<Pin<Box<dyn Future<Output = Result<Vec<Row>, crate::Error>>>>>>
}

impl<T: TryFrom<Row, Error = crate::Error>> SelectBuilder<T> {
    pub(crate) fn new(cols: &[&Column]) -> SelectBuilder<T> {
        let table = cols[0].table_name().to_string();
        let cols = cols.iter().map(|i| i.full_name()).collect::<Vec<_>>().join(", ");
        SelectBuilder { 
            cols,
            table, 
            filter: Filter::all(), 
            parse_to: PhantomData::<T>,
            fut: Mutex::new(None)
        }
    }

    fn to_stmt(&self) -> String {
        
        format!(
            "SELECT {} FROM {} {}", 
            self.cols,
            self.table,
            self.filter.to_sql()
        )
    }

    /// Add a WHERE clause to your query:
    /// 
    /// ```
    /// use pg_worm::prelude::*;
    /// #[derive(Model)]
    /// struct Book {
    ///     id: i32,
    ///     title: String
    /// }
    /// 
    /// async some_func() {
    ///     let og_book = Book::select()
    ///         .filter(Book::id.eq(1))
    ///         .await.unwrap();
    /// }
    /// ```
    pub fn filter(mut self, filter: Filter) -> SelectBuilder<T> {
        self.filter = self.filter & filter;

        self
    }

    pub async fn exec(self) -> Result<Vec<T>, pg_worm::Error> {
        let stmt = self.to_stmt();

        // Prepare params
        let params = self.filter.args()
            .iter()
            .map(|i| &**i as &(dyn ToSql + Sync))
            .collect::<Vec<_>>();

        let client = _get_client()?;

        let Ok(res) = client.query(&stmt, &params).await else {
            return Err(crate::Error::ConnectionError)
        };

        res 
            .into_iter()
            .map(T::try_from)
            .collect()
    }
}
