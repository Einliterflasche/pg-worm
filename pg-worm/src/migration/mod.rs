//! This module contains the logic needed to create automatic migrations.

use std::fmt::Display;

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
    constraints: Vec<TableConstraint>
}

#[derive(Debug, Clone)]
struct Column {
    name: String,
    data_type: String,
    constraints: Vec<ColumnConstraint>,
}

#[derive(Debug, Clone)]
enum ColumnConstraint {
    Unique,
    UniqueNamed(String),
    NotNull,
    PrimaryKey,
    ForeignKey(String, String),
    ForeignKeyNamed(String, String, String),
    RawCheck(String),
    RawCheckNamed(String, String)
}

#[derive(Debug, Clone)]
enum TableConstraint {
    PrimaryKey(Vec<String>),
    ForeignKey(String, Vec<(String, String)>),
    ForeignKeyNamed(String, String, Vec<(String, String)>),
    Unique(Vec<String>),
    UniqueNamed(String, Vec<String>),
    RawCheck(String),
    RawCheckNamed(String, String)
}

impl Table {
    fn new(name: impl Into<String>) -> Self {
        let columns: Vec<Column> = Vec::new();

        Table {
            name: name.into(),
            columns,
            constraints: Vec::new()
        }
    }

    fn column(mut self, col: Column) -> Table {
        self.columns.push(col);

        self
    }
}

impl Column {
    fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Column {
            name: name.into(),
            data_type: data_type.into(),
            constraints: Vec::new(),
        }
    }

    fn not_null(mut self) -> Self {
        self.constraints.push(ColumnConstraint::NotNull);

        self
    }

    fn unique(mut self) -> Self {
        self.constraints.push(ColumnConstraint::Unique);

        self
    }

    fn unique_named(mut self, name: String) -> Self {
        self.constraints.push(ColumnConstraint::UniqueNamed(name));

        self
    }

    fn primary_key(mut self) -> Self {
        self.constraints.push(ColumnConstraint::PrimaryKey);

        self
    }

    fn foreign_key(mut self, table_name: String, column_name: String) -> Self {
        self.constraints
            .push(ColumnConstraint::ForeignKey(table_name, column_name));

        self
    }
}

trait Up {
    fn up(&self) -> String;
}

impl Up for Table {
    fn up(&self) -> String {
        let mut up = format!(
            "CREATE TABLE {} ({})",
            self.name,
            self.columns.iter().map(|i| i.up())._join(", ")
        );

        if !self.constraints.is_empty() {
            up.push_str(&format!(
                ", {}",
                self.constraints.iter().map(|i| i.up())._join(", ")
            ));
        }

        up
    }
}

impl Up for Column {
    fn up(&self) -> String {
        let mut up = format!("{} {}", self.name, self.data_type);
        
        if !self.constraints.is_empty() {
            up.push(' ');
            up.push_str(&self.constraints
                .iter()
                .map(|i| i.up())
                .collect::<Vec<String>>()
                ._join(" "));
        }

        up
    }
}

impl Up for TableConstraint {
    fn up(&self) -> String {
        use TableConstraint as C;

        match self {
            C::PrimaryKey(cols) => format!("PRIMARY KEY ({})", cols._join(", ")),
            C::ForeignKey(table, cols) => format!(
                "FOREIGN KEY ({}) REFERENCES {table} ({})", 
                cols.iter().map(|i| &i.0)._join(", "),
                cols.iter().map(|i| &i.1)._join(", ")
            ),
            C::ForeignKeyNamed(name, table, cols) => format!(
                "CONSTRAINT {name} FOREIGN KEY ({}) REFERENCES {table} ({})", 
                cols.iter().map(|i| &i.0)._join(", "),
                cols.iter().map(|i| &i.1)._join(", ")
            ),
            C::Unique(cols) => format!("UNIQUE ({})", cols.join(", ")),
            C::UniqueNamed(name, cols) => format!("CONSTRAINT {name} UNIQUE ({})", cols.join(", ")),
            C::RawCheck(check) => format!("CHECK ({check})"),
            C::RawCheckNamed(name, check) => format!("CONSTRAINT {name} CHECK ({check})"),
        }
    }
}

impl Up for ColumnConstraint {
    fn up(&self) -> String {
        use ColumnConstraint as C;

        match self {
            C::NotNull => format!("NOT NULL"),
            C::PrimaryKey => format!("PRIMARY KEY"),
            C::ForeignKeyNamed(name, table, col) => format!("CONSTRAINT {name} REFERENCES {table} ({col}"),
            C::ForeignKey(table, col) => format!("REFERENCES {table} ({col})"),
            C::Unique => format!("UNIQUE"),
            C::UniqueNamed(name) => format!("CONSTRAINT {name} UNIQUE"),
            C::RawCheck(check) => format!("CHECK ({check})"),
            C::RawCheckNamed(name, check) => format!("CONSTRAINT {name} CHECK ({check})")
        }
    }
}

trait Join {
    fn _join(self, _: &str) -> String;
}

impl<T, U> Join for T
where 
    T: IntoIterator<Item = U>,
    U: Display
{
    fn _join(self, sep: &str) -> String {
        (*self.into_iter().map(|i| i.to_string()).collect::<Vec<String>>()).join(sep)
    }
}

#[cfg(test)]
mod tests {
    use crate::migration::Up;

    use super::{Table, Column};

    #[test]
    fn migrate() {
        let dest = Table::new("book")
            .column(Column::new("id", "BIGINT").primary_key().not_null())
            .column(Column::new("title", "TEXT").unique().not_null());

        dbg!(dest.up());
    }
}
