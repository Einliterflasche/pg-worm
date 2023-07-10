use std::future::{IntoFuture, Future};
use std::pin::Pin;

use crate::{Filter, _get_client, conv_params};
use crate::pg::types::ToSql;

pub struct DeleteBuilder {
    table: &'static str,
    filter: Filter,
}

impl DeleteBuilder {
    pub(crate) fn new(table: &'static str) -> DeleteBuilder {
        DeleteBuilder {
            table,
            filter: Filter::all()
        }
    }

    pub fn filter(mut self, filter: Filter) -> DeleteBuilder {
        self.filter = self.filter & filter;

        self
    }

    pub async fn exec(self) -> Result<u64, crate::Error> {
        let client = _get_client()?;
        let stmt = format!(
            "DELETE FROM {} {}",
            self.table,
            self.filter.to_sql()
        );
        let params = conv_params!(self.filter.args());

        let res = client.execute(&stmt, params.as_slice()).await?;

        Ok(res)
    }
}

impl IntoFuture for DeleteBuilder {
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;
    type Output = Result<u64, crate::Error>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(
            async move {
                self.exec().await
            }
        )
    }
}
