use std::{marker::PhantomData, ops::Deref};

use tokio_postgres::types::ToSql;

use crate::Filter;

pub struct Column<T: ToSql + Sync> {
    shared: ColumnShared,
    rs_type: PhantomData<T>,
}

#[derive(Copy, Clone)]
pub struct ColumnShared {
    column_name: &'static str,
    table_name: &'static str,
}

impl<T: ToSql + Sync + Send + 'static> Column<T> {
    pub const fn new(table_name: &'static str, column_name: &'static str) -> Column<T> {
        Column {
            shared: ColumnShared {
                table_name,
                column_name,
            },
            rs_type: PhantomData::<T>,
        }
    }

    pub const fn name(&self) -> &'static str {
        self.shared.column_name
    }
    /// Check whether the columns value is equal to `value`.
    ///
    /// Translates to `WHERE <column_name> = <value>`.
    pub fn eq(&self, value: impl Into<T>) -> Filter {
        Filter::new(
            format!("{} = $1", self.shared.column_name),
            vec![Box::new(value.into())],
        )
    }

    /// Check whether the columns value is one of `values`.
    ///
    /// Translates to `WHERE <column_name> IN <values>`
    pub fn one_of(&self, values: Vec<impl Into<T>>) -> Filter {
        // Early return if no values are supplied
        if values.is_empty() {
            return Filter::all();
        }

        // Generate the placeholders for the query
        // like $1, $2, ...
        let placeholders = (1..=values.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");

        // Convert values to needed type
        let vals = values
            .into_iter()
            .map(|i| Box::new(i.into()) as Box<(dyn ToSql + Send + Sync + 'static)>)
            .collect::<Vec<_>>();

        Filter::new(
            format!("{} IN ({placeholders})", self.shared.column_name),
            vals,
        )
    }

    pub fn is_null(&self) -> Filter {
        Filter::new(format!("{} IS NULL", self.shared.column_name), Vec::new())
    }
}

impl<T: ToSql + Sync> Deref for Column<T> {
    type Target = ColumnShared;

    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

impl ColumnShared {
    pub const fn column_name(&self) -> &'static str {
        self.column_name
    }

    pub const fn table_name(&self) -> &'static str {
        self.table_name
    }
}
