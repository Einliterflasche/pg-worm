use std::{
    future::{Future, IntoFuture},
    marker::PhantomData,
    pin::Pin,
};

use tokio_postgres::{types::ToSql, Row};

use crate::{Column, Filter, _get_client, conv_params};

#[must_use]
pub struct SelectBuilder<T> {
    cols: String,
    table: String,
    filter: Filter,
    parse_to: PhantomData<T>,
}

impl<T> SelectBuilder<T> {
    pub(crate) fn new(cols: &[&Column]) -> SelectBuilder<T> {
        let table = cols[0].table_name().to_string();
        let cols = cols
            .iter()
            .map(|i| i.full_name())
            .collect::<Vec<_>>()
            .join(", ");
        SelectBuilder {
            cols,
            table,
            filter: Filter::all(),
            parse_to: PhantomData::<T>,
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
    /// ```ignore
    /// use pg_worm::prelude::*;
    /// #[derive(Model)]
    /// struct Book {
    ///     id: i32,
    ///     title: String
    /// }
    ///
    /// let og_book = Book::select()
    ///     .filter(Book::id.eq(1))
    ///     .await?;
    /// ```
    pub fn filter(mut self, filter: Filter) -> SelectBuilder<T> {
        self.filter = self.filter & filter;

        self
    }
}

impl<T: TryFrom<Row, Error = crate::Error>> SelectBuilder<Vec<T>> {
    pub async fn exec(self) -> Result<Vec<T>, pg_worm::Error> {
        let stmt = self.to_stmt();

        // Prepare params
        let params = conv_params!(self.filter.args());

        let client = _get_client()?;

        let res = client.query(&stmt, &params).await?;

        res.into_iter().map(T::try_from).collect()
    }
}

impl<T: TryFrom<Row, Error = crate::Error>> SelectBuilder<Option<T>> {
    pub async fn exec(self) -> Result<Option<T>, crate::Error> {
        let stmt = self.to_stmt();
        let params = conv_params!(self.filter.args());
        let client = _get_client()?;

        let res = client
            .query(&stmt, &params)
            .await?
            .into_iter()
            .map(|i| T::try_from(i))
            .next();

        match res {
            None => Ok(None),
            Some(res) => match res {
                Ok(res) => Ok(Some(res)),
                Err(err) => Err(err),
            },
        }
    }
}

impl<T: TryFrom<Row, Error = crate::Error> + 'static> IntoFuture for SelectBuilder<Vec<T>> {
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;
    type Output = Result<Vec<T>, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.exec().await })
    }
}

impl<T: TryFrom<Row, Error = crate::Error> + 'static> IntoFuture for SelectBuilder<Option<T>> {
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;
    type Output = Result<Option<T>, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.exec().await })
    }
}
