//! This module contains the logic needed to create automatic migrations.

#![allow(dead_code)]

use std::{fmt::Display, ops::Deref};

use hashbrown::HashMap;
use tokio_postgres::Row;

use crate::{pool::fetch_client, FromRow};

/// Represents a collection of tables.
#[derive(Debug, Clone)]
pub struct Schema {
    name: String,
    tables: Vec<Table>,
}

/// Represents a table.
#[derive(Debug, Clone)]
pub struct Table {
    name: String,
    columns: Vec<Column>,
    constraints: Vec<TableConstraint>,
}

/// Represents a column.
#[derive(Debug, Clone)]
pub struct Column {
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

/// Fetch a schema from a database connection.
///
/// May fail due to connection errors, parsing errors or
/// when querying for a not existing schema.
async fn fetch_schema(
    schema_name: impl Into<String>,
    client: &tokio_postgres::Client,
) -> Result<Schema, crate::Error> {
    let schema_name = schema_name.into();

    struct Entry {
        table: String,
        column: String,
        data_type: String,
    }

    impl TryFrom<Row> for Entry {
        type Error = crate::Error;

        fn try_from(row: Row) -> Result<Self, Self::Error> {
            Ok(Entry {
                table: row
                    .try_get("table")
                    .map_err(|_| crate::Error::ParseError("Entry", "table"))?,
                column: row.try_get("column")?,
                data_type: row.try_get("data_type")?,
            })
        }
    }

    impl FromRow for Entry {}

    // Query all columns and their data type of all tables in this schema
    let res = client
        .query(
            r#"
            SELECT
                pg_attribute.attname AS column,
                pg_catalog.format_type(pg_attribute.atttypid, pg_attribute.atttypmod) AS data_type,
                pg_class.relname AS table
            FROM
                pg_catalog.pg_attribute
            INNER JOIN
                pg_catalog.pg_class ON pg_class.oid = pg_attribute.attrelid
            INNER JOIN
                pg_catalog.pg_namespace ON pg_namespace.oid = pg_class.relnamespace
            WHERE
                pg_attribute.attnum > 0
                AND NOT pg_attribute.attisdropped
                AND pg_namespace.nspname = $1
                AND pg_class.relkind = 'r'
            ORDER BY
                attnum ASC
        "#,
            &[&schema_name],
        )
        .await?;

    // Parse the query result to `Entry` objects.
    let entries: Vec<Entry> = res
        .into_iter()
        .map(Entry::try_from)
        .collect::<Result<Vec<_>, crate::Error>>()?;

    // Group the columns by table. No idea if there's a better way to do this
    let mut map: HashMap<String, Vec<Entry>> = HashMap::new();
    for i in entries {
        if let Some(columns) = map.get_mut(&i.table) {
            columns.push(i);
        } else {
            map.insert(i.table.clone(), vec![i]);
        }
    }

    let tables = map.into_iter().map(|(table, columns)| {
        Table::new(table).columns(
            columns
                .into_iter()
                .map(|i| Column::new(i.column, i.data_type)),
        )
    });

    let schema = Schema::new(schema_name).tables(tables);

    Ok(schema)
}

/// Try to automatically migrate from `old` to `new`.
pub async fn try_migration_from(
    old: &Schema,
    new: &Schema,
    client: &tokio_postgres::Client,
) -> Result<(), crate::Error> {
    let stmts = old.migrate_from(new)._join("; ");

    client
        .simple_query(&stmts)
        .await
        .map(|_| ())
        .map_err(crate::Error::PostgresError)
}

/// Try to fetch the current schema and then automatically
/// migrate from that to `new`.
pub async fn try_migration_to(
    new: &Schema,
    client: &tokio_postgres::Client,
) -> Result<(), crate::Error> {
    let old = fetch_schema(&new.name, client).await?;
    try_migration_from(&old, new, client).await
}

/// Automatically create new or alter existing tables in the `'public'` schema.
pub async fn migrate_tables(table: impl IntoIterator<Item = Table>) -> Result<(), crate::Error> {
    let new = Schema::default().tables(&mut table.into_iter());
    try_migration_to(&new, fetch_client().await?.deref()).await
}

impl Default for Schema {
    fn default() -> Self {
        Schema {
            name: "public".into(),
            tables: Vec::new(),
        }
    }
}

impl Schema {
    /// Create a new schema.
    pub fn new(name: impl Into<String>) -> Schema {
        Self {
            name: name.into(),
            tables: Vec::new(),
        }
    }

    /// Add a table to this schema.
    pub fn table(mut self, table: Table) -> Self {
        self.tables.push(table);

        self
    }

    /// Add multiple tables to this schema.
    pub fn tables(mut self, tables: impl IntoIterator<Item = Table>) -> Self {
        self.tables.extend(&mut tables.into_iter());

        self
    }

    /// Generate SQL statements which migrate `old` to this schema.
    fn migrate_from(&self, old: &Schema) -> Vec<String> {
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
    /// Create a new table.
    pub fn new(name: impl Into<String>) -> Self {
        let columns: Vec<Column> = Vec::new();

        Table {
            name: name.into(),
            columns,
            constraints: Vec::new(),
        }
    }

    /// Add a column to this table.
    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);

        self
    }

    /// Add columns to this table.
    pub fn columns(mut self, columns: impl IntoIterator<Item = Column>) -> Self {
        self.columns.extend(&mut columns.into_iter());

        self
    }

    /// Add a unique constraint to column(s) of this table.
    pub fn unique(mut self, cols: impl IntoIterator<Item = String>) -> Self {
        self.constraints
            .push(TableConstraint::Unique(cols.into_iter().collect()));

        self
    }

    /// Add a named unique constraint to column(s) of this table.
    pub fn unique_named(
        mut self,
        name: impl Into<String>,
        cols: impl IntoIterator<Item = String>,
    ) -> Self {
        self.constraints.push(TableConstraint::UniqueNamed(
            name.into(),
            cols.into_iter().collect(),
        ));

        self
    }

    /// Add a primary key to this table.
    pub fn primary_key(mut self, cols: impl IntoIterator<Item = String>) -> Self {
        self.constraints
            .push(TableConstraint::PrimaryKey(cols.into_iter().collect()));

        self
    }

    /// Add a foreign key constraint to this table.
    pub fn foreign_key(
        mut self,
        table: impl Into<String>,
        columns: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.constraints.push(TableConstraint::ForeignKey(
            table.into(),
            columns.into_iter().collect(),
        ));

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

        for i in old_table
            .columns
            .iter()
            .filter(|i| !self.columns.iter().any(|j| i.name == j.name))
        {
            statements.push(format!("ALTER TABLE {} {}", self.name, i.down()));
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
                        WHERE rel.relname = '{0}') LOOP
                    EXECUTE format('ALTER TABLE {0} DROP CONSTRAINT %I CASCADE', i.conname);
                END LOOP;
            END $$"#,
            self.name
        )
    }
}

impl Column {
    /// Create a new column.
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Column {
            name: name.into(),
            data_type: data_type.into(),
            constraints: Vec::new(),
        }
    }

    /// Make this column `NOT NULL`.
    pub fn not_null(mut self) -> Self {
        self.constraints.push(ColumnConstraint::NotNull);

        self
    }

    /// Add a `UNIQUE` constraint to this column.
    pub fn unique(mut self) -> Self {
        self.constraints.push(ColumnConstraint::Unique);

        self
    }

    /// Add a named `UNIQUE` constraint to this column.
    pub fn unique_named(mut self, name: String) -> Self {
        self.constraints.push(ColumnConstraint::UniqueNamed(name));

        self
    }

    /// Make this column the `PRIMARY KEY`.
    pub fn primary_key(mut self) -> Self {
        self.constraints.push(ColumnConstraint::PrimaryKey);

        self
    }

    /// Add a `FOREIGN KEY` constraint to this column.
    pub fn foreign_key(mut self, table_name: String, column_name: String) -> Self {
        self.constraints
            .push(ColumnConstraint::ForeignKey(table_name, column_name));

        self
    }

    /// Add a raw `CHECK` to this column.
    pub fn check(mut self, check: impl Into<String>) -> Self {
        self.constraints
            .push(ColumnConstraint::RawCheck(check.into()));

        self
    }

    /// Add a named raw `CHECK` to this column.
    pub fn check_named(mut self, name: impl Into<String>, check: impl Into<String>) -> Self {
        self.constraints
            .push(ColumnConstraint::RawCheckNamed(name.into(), check.into()));

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
                "ALTER COLUMN {} TYPE {}",
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
    use crate::pool::{fetch_client, Connection};

    use super::{fetch_schema, try_migration_from, Column, Schema, Table};

    #[tokio::test]
    async fn migrate() -> Result<(), Box<dyn std::error::Error>> {
        Connection::build("postgres://postgres:postgres@localhost:5432")
            .connect()
            .await
            .unwrap();
        let src = fetch_schema("public", &&fetch_client().await.unwrap())
            .await
            .unwrap();
        let dest = Schema::default().table(
            Table::new("book")
                .column(Column::new("id", "BIGINT").primary_key().not_null())
                .column(Column::new("title", "TEXT").unique().not_null()),
        );

        try_migration_from(&src, &dest, &&fetch_client().await?).await?;

        Ok(())
    }
}
