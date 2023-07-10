use std::{future::{IntoFuture, Future}, pin::Pin};

use tokio_postgres::types::ToSql;

use crate::{TypedColumn, _get_client, Filter};

pub struct UpdateBuilder {
    table: &'static str,
    updates: Vec<Update>
}

struct Update(&'static str, Box<dyn ToSql + Sync>);

impl UpdateBuilder {
    pub(crate) fn new(table: &'static str) -> UpdateBuilder {
        UpdateBuilder { table, updates: vec![] }
    }

    pub fn set<T: ToSql + Sync + 'static>(mut self, column: TypedColumn<T>, value: impl Into<T>) -> UpdateBuilder {
        let update = Update(column.column_name(), Box::new(value.into()));
        self.updates.push(update);

        self
    }

    pub async fn exec(self) -> Result<u64, crate::Error> {
        if self.updates.len() == 0 {
            return Err(crate::Error::NoUpdates(self.table.into()))
        }

        let updates = self.updates.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let client = _get_client()?;
        let stmt = format!(
            "UPDATE {} SET {}",
            self.table,
            updates
        );
        let stmt = Filter::question_mark_to_numbered_dollar(stmt);

        let params = self.updates.iter()
            .map(|i| &*(i.1) as &(dyn ToSql + Sync))
            .collect::<Vec<_>>();

        let res = client.execute(&stmt, params.as_slice()).await?;

        Ok(res)   
    }
}

impl IntoFuture for UpdateBuilder {
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

impl ToString for Update {
    fn to_string(&self) -> String {
        format!("{} = ?", self.0)
    }
}

