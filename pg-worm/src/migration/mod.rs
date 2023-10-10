//! This module contains the logic needed to create automatic migrations.

#![allow(dead_code)]

use std::fmt::Display;

/// Represents a collection of tables.
#[derive(Default, Debug, Clone)]
pub struct Schema {
    tables: Vec<Table>,
}

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

/// Constraints which may be placed on a table.
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

/// Constraints which may be placed on a column.
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

impl Schema {
    /// Add a table to this schema.
    fn table(mut self, table: Table) -> Schema {
        self.tables.push(table);

        self
    }

    /// Generate SQL statements which migrate `old` to this schema.
    pub fn migrate_from(&self, old: &Schema) -> Vec<String> {
        let mut statements = Vec::new();
        for table in &self.tables {
            if let Some(old_table) = old.tables.iter().find(|i| i.name == table.name) {
                statements.append(&mut table.migrate_without_constraints(old_table));
            } else {
                statements.push(table.up());
            }
        }

        // Add all constraints only after creating the tables to make sure
        // the referenced columns exist.
        let mut tmp = self
            .tables
            .iter()
            .flat_map(|i| i.add_constraints())
            .collect::<Vec<String>>();

        statements.append(&mut tmp);

        statements
    }
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

    fn migrate_without_constraints(&self, old_table: &Table) -> Vec<String> {
        let mut statements = Vec::new();

        // If there are any constraints on the old table, drop them
        statements.push(self.drop_all_constraints_cascading());

        for new_column in &self.columns {
            if let Some(old_column) = old_table.columns.iter().find(|i| i.name == new_column.name) {
                // If a column of the same name already exists, change it.

                let mut stmts = new_column
                    .migrate_without_constraints(old_column)
                    .iter()
                    .map(|i| format!("ALTER TABLE {} {}", self.name, i))
                    .collect::<Vec<String>>();
                statements.append(&mut stmts);
            } else {
                // Else, create a new column
                statements.push(format!("ALTER TABLE {} {}", self.name, new_column.up()));
            }
        }

        statements
    }

    fn add_constraints(&self) -> Vec<String> {
        let mut statements = Vec::new();

        for i in &self.columns {
            let mut stmts = i
                .add_constraints()
                .iter()
                .map(|i| format!("ALTER TABLE {} {}", self.name, i))
                .collect::<Vec<String>>();

            statements.append(&mut stmts);
        }

        for i in &self.constraints {
            statements.push(format!("ALTER TABLE {} {}", self.name, i.migrate_to()));
        }

        statements
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
        format!("{} {}", self.name, self.data_type)
    }

    fn down(&self) -> String {
        format!("DROP COLUMN IF EXISTS {}", self.name)
    }

    fn migrate_without_constraints(&self, other: &Column) -> Vec<String> {
        let mut statements = Vec::new();

        if self.data_type != other.data_type {
            statements.push(format!(
                "ALTER COLUMN {} SET TYPE {}",
                self.name, self.data_type
            ));
        }

        statements
    }

    fn add_constraints(&self) -> Vec<String> {
        self.constraints
            .iter()
            .map(|i| i.migrate_to(self))
            .collect()
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

    fn migrate_to(&self) -> String {
        use TableConstraint as T;

        match self {
            T::PrimaryKey(columns) => {
                format!("ADD CONSTRAINT PRIMARY KEY ({})", columns.join(", "))
            }
            T::Unique(columns) => format!("ADD UNIQUE ({})", columns.join(", ")),
            T::UniqueNamed(name, columns) => {
                format!("ADD CONSTRAINT {name} UNIQUE ({})", columns.join(", "))
            }
            T::ForeignKey(table, columns) => format!(
                "ADD FOREIGN KEY ({}) REFERENCES {table} ({})",
                columns.iter().map(|i| &i.0)._join(", "),
                columns.iter().map(|i| &i.1)._join(", ")
            ),
            T::ForeignKeyNamed(name, table, columns) => format!(
                "ADD CONSTRAINT {name} FOREIGN KEY ({}) REFERENCES {table} ({})",
                columns.iter().map(|i| &i.0)._join(", "),
                columns.iter().map(|i| &i.1)._join(", ")
            ),
            T::RawCheck(check) => format!("ADD CHECK ({check})"),
            T::RawCheckNamed(name, check) => format!("ADD CONSTRAINT {name} CHECK ({check})"),
        }
    }
}

impl ColumnConstraint {
    fn up(&self) -> String {
        use ColumnConstraint as C;

        match self {
            C::NotNull => "NOT NULL".to_string(),
            C::PrimaryKey => "PRIMARY KEY".to_string(),
            C::ForeignKey(table, col) => format!("REFERENCES {table} ({col})"),
            C::ForeignKeyNamed(name, table, col) => {
                format!("CONSTRAINT {name} REFERENCES {table} ({col}")
            }
            C::Unique => "UNIQUE".to_string(),
            C::UniqueNamed(name) => format!("CONSTRAINT {name} UNIQUE"),
            C::RawCheck(check) => format!("CHECK ({check})"),
            C::RawCheckNamed(name, check) => format!("CONSTRAINT {name} CHECK ({check})"),
        }
    }

    fn migrate_to(&self, column: &Column) -> String {
        use ColumnConstraint as C;

        match self {
            C::NotNull => format!("ALTER COLUMN {} SET NOT NULL", column.name),
            C::PrimaryKey => format!("ADD CONSTRAINT PRIMARY KEY ({})", column.name),
            C::Unique => format!("ALTER COLUMN {} SET UNIQUE", column.name),
            C::UniqueNamed(name) => format!("ADD CONSTRAINT {name} UNIQUE ({})", column.name),
            C::ForeignKey(table, other_column) => format!(
                "ADD FOREIGN KEY ({}) REFERENCES {table} ({other_column})",
                column.name
            ),
            C::ForeignKeyNamed(name, table, other_column) => format!(
                "ADD CONSTRAINT {name} FOREIGN KEY ({}) REFERENCES {table} ({other_column})",
                column.name
            ),
            C::RawCheck(check) => format!("ADD CHECK ({check})"),
            C::RawCheckNamed(name, check) => format!("ADD CONSTRAINT {name} CHECK ({check})"),
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
    use super::{Column, Schema, Table};

    #[test]
    fn migrate() {
        let dest = Schema::default().table(
            Table::new("book")
                .column(Column::new("id", "BIGINT").primary_key().not_null())
                .column(Column::new("title", "TEXT").unique().not_null()),
        );

        dbg!(dest.migrate_from(&Schema::default()));
    }
}
