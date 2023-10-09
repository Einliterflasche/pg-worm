//! This module contains the logic needed to create automatic migrations.

#![allow(dead_code)]

use std::fmt::Display;

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
    constraints: Vec<TableConstraint>,
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
    RawCheckNamed(String, String),
}

#[derive(Debug, Clone)]
enum TableConstraint {
    PrimaryKey(Vec<String>),
    ForeignKey(String, Vec<(String, String)>),
    ForeignKeyNamed(String, String, Vec<(String, String)>),
    Unique(Vec<String>),
    UniqueNamed(String, Vec<String>),
    RawCheck(String),
    RawCheckNamed(String, String),
}

impl Table {
    fn new(name: impl Into<String>) -> Self {
        let columns: Vec<Column> = Vec::new();

        Table {
            name: name.into(),
            columns,
            constraints: Vec::new(),
        }
    }

    fn column(mut self, col: Column) -> Table {
        self.columns.push(col);

        self
    }

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

    fn down(&self) -> String {
        format!("DROP TABLE IF EXISTS {}", self.name)
    }

    fn drop_all_constraints_cascading(&self) -> String {
        // This is a pl/pgsql code block which first queries for all
        // constraints on a given table and then removes them.
        format!(
            r#"DO $$
                DECLARE i RECORD;
                BEGIN
                    FOR i IN (SELECT conname
                        FROM pg_catalog.pg_constraint con
                        INNER JOIN pg_catalog.pg_class rel ON rel.oid = con.conrelid
                        INNER JOIN pg_catalog.pg_namespace nsp ON nsp.oid = connamespace
                        WHERE rel.relname = {0}) LOOP
                    EXECUTE format('ALTER TABLE {0} DROP CONSTRAINT %I CASCADE', i.conname);
                END LOOP;
            END $$;"#,
            self.name
        )
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

    fn up(&self) -> String {
        let mut up = format!("{} {}", self.name, self.data_type);

        if !self.constraints.is_empty() {
            up.push(' ');
            up.push_str(
                &self
                    .constraints
                    .iter()
                    .map(|i| i.up())
                    .collect::<Vec<String>>()
                    ._join(" "),
            );
        }

        up
    }

    fn down(&self) -> String {
        format!("DROP COLUMN IF EXISTS {}", self.name)
    }

    fn migrate_from(&self, other: &Column, table: &Table) -> Vec<String> {
        let mut statements = Vec::new();

        if other.constraints.len() > 0 {
            statements.push(table.drop_all_constraints_cascading());
        }

        if self.constraints.len() > 0 {
            let mut stmts = self
                .constraints
                .iter()
                .map(|i| i.migrate_to(&self))
                .collect::<Vec<String>>();

            statements.append(&mut stmts);
        }

        if self.data_type != other.data_type {
            statements.push(format!(
                "ALTER COLUMN {} SET TYPE {}",
                self.name, self.data_type
            ));
        }

        statements
    }
}

impl TableConstraint {
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

impl ColumnConstraint {
    fn up(&self) -> String {
        use ColumnConstraint as C;

        match self {
            C::NotNull => format!("NOT NULL"),
            C::PrimaryKey => format!("PRIMARY KEY"),
            C::ForeignKeyNamed(name, table, col) => {
                format!("CONSTRAINT {name} REFERENCES {table} ({col}")
            }
            C::ForeignKey(table, col) => format!("REFERENCES {table} ({col})"),
            C::Unique => format!("UNIQUE"),
            C::UniqueNamed(name) => format!("CONSTRAINT {name} UNIQUE"),
            C::RawCheck(check) => format!("CHECK ({check})"),
            C::RawCheckNamed(name, check) => format!("CONSTRAINT {name} CHECK ({check})"),
        }
    }

    fn migrate_to(&self, column: &Column) -> String {
        use ColumnConstraint as C;

        match self {
            C::NotNull => format!("ALTER COLUMN {} DROP NULL", column.name),
            C::PrimaryKey => format!("ADD CONSTRAINT PRIMARY KEY ({})", column.name),
            C::Unique => format!("ALTER COLUMN {} SET UNIQUE", column.name),
            _ => todo!(),
        }
    }
}

/// Convenience method to avoid writing `.iter().map(|i| i.to_string()).collect::<Vec<String>>().join()`
/// all the time.
trait Join {
    fn _join(self, _: &str) -> String;
}

impl<T, U> Join for T
where
    T: IntoIterator<Item = U>,
    U: Display,
{
    fn _join(self, sep: &str) -> String {
        (*self
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>())
        .join(sep)
    }
}

#[cfg(test)]
mod tests {
    use super::{Column, Table};

    #[test]
    fn migrate() {
        let dest = Table::new("book")
            .column(Column::new("id", "BIGINT").primary_key().not_null())
            .column(Column::new("title", "TEXT").unique().not_null());

        dbg!(dest.up());
    }
}
