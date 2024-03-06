//! This module contains the logic needed to create automatic migrations.
use std::fmt::Display;

use thiserror::Error;
use tokio_postgres::Row;

use crate::pool::Client;

#[derive(Debug, Error)]
pub enum MigrationError {
    /// When data is missing but shouldn't be or of a
    /// different type than expected.
    #[error("error parsing query output")]
    ParsingError(tokio_postgres::Error),
    /// When data is there and of the correct type but
    /// has an unexpected value.
    #[error("unknown and unexpected value")]
    UnexpectedValue(String),
}

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
    constraints: Vec<Constraint>,
}

/// Represents a column.
#[derive(Debug, Clone)]
pub struct Column {
    name: String,
    data_type: String,
    not_null: bool,
}

/// Represents a constraint on a table such as `UNIQUE`, `PRIMARY KEY`,
/// `FOREIGN KEY` or `CHECK`.
#[derive(Debug, Clone, Hash)]
pub struct Constraint {
    constraint_name: String,
    table_name: String,
    constraint_type: ConstraintType,
}

/// The different types a constraint can be of plus their respective variables.
///
/// Implements `PartialOrd` so that `PRIMARY KEY`s and `UNIQUE` constraints
/// are added first in order to make the `FOREIGN KEY`s possable.
#[derive(Debug, Clone, Hash, PartialEq, PartialOrd)]
pub enum ConstraintType {
    /// A primary key over one or more columns.
    PrimaryKey {
        /// The columns this primary key is made of.
        columns: Vec<String>,
    },
    /// A uniqueness constraint over one or more columns.
    Unique {
        /// The columns that have to make a unique combination
        columns: Vec<String>,
    },
    /// A foreign key over one or more columns.
    /// `columns` and `foreign_columns` must be the same length.
    ForeignKey {
        /// The columns of this table which are mapped to foreign columns.
        columns: Vec<String>,
        /// The name of the foreign table.
        foreign_table: String,
        /// The foreign columns.
        foreign_columns: Vec<String>,
    },
    /// An arbitrary check constraint.
    Check {
        /// The definition of the constraint
        definition: String,
    },
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

    /// Fetch a specific schema
    pub async fn fetch(client: Client, schema_name: &str) -> Result<Option<Schema>, crate::Error> {}
}

impl Table {
    /// Create a new table.
    pub fn new(name: impl Into<String>) -> Self {
        Table {
            name: name.into(),
            columns: Vec::new(),
            constraints: Vec::new(),
        }
    }

    /// Add a column to this table.
    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);

        self
    }

    /// Add a constraint to this table.
    pub fn constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);

        self
    }
}

impl Column {
    /// Create a new column.
    pub fn new(name: impl Into<String>, data_type: impl Into<String>, not_null: bool) -> Self {
        Column {
            name: name.into(),
            data_type: data_type.into(),
            not_null,
        }
    }

    /// Generates the expression used to create this column.
    ///
    /// Of the form `{name} {data_type} {not_null?}`
    fn add_column_expression(&self) -> String {
        let mut expr = format!("{} {}", self.name, self.data_type);

        if self.not_null {
            expr.push_str(" NOT NULL");
        }

        expr
    }
}

impl Constraint {
    /// Returns part of the statement to
    fn add_statement(&self) -> String {
        let mut expr = format!("CONSTRAINT {} ", self.constraint_name);

        use ConstraintType as T;

        expr.push_str(&match &self.constraint_type {
            T::Check { definition } => format!("CHECK {definition}"),
            T::Unique { columns } => format!("UNIQUE ({})", columns.join(", ")),
            T::PrimaryKey { columns } => format!("PRIMARY KEY ({})", columns.join(", ")),
            T::ForeignKey {
                columns,
                foreign_table,
                foreign_columns,
            } => format!(
                "FOREIGN KEY ({}) REFERENCES {foreign_table} ({})",
                columns.join(", "),
                foreign_columns.join(", ")
            ),
        });

        expr
    }
}

/// Convenience method to avoid writing `iter().map(|i| i.to_string()).collect::<Vec<String>>().join()`
/// all the time.
trait Join {
    fn my_join(self, _: impl AsRef<str>) -> String;
}

impl<T, U> Join for T
where
    T: IntoIterator<Item = U>,
    U: Display,
{
    fn my_join(self, sep: impl AsRef<str>) -> String {
        (*self
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>())
        .join(sep.as_ref())
    }
}

/// A module which groups functions for generating sql statements.
mod sql {
    use crate::migration::Join;

    use super::{Column, Constraint, Table};

    pub fn set_column_type(table: &str, column: &str, ty: &str) -> String {
        format!("ALTER TABLE {table} ALTER COLUMN {column} TYPE {ty}")
    }

    pub fn add_constraint(table: &str, constraint: Constraint) -> String {
        format!("ALTER TABLE {table} ADD {}", constraint.add_statement())
    }

    pub fn drop_constraint(table: &str, constraint_name: &str) -> String {
        format!("ALTER TABLE {table} DROP CONSTRAINT {constraint_name}")
    }

    pub fn change_column_not_null(table: &str, column: &str, not_null: bool) -> String {
        let action = if not_null { "SET" } else { "DROP" };
        format!("ALTER TABLE {table} ALTER COLUMN {column} {action} NOT NULL")
    }

    pub fn add_column(table: &str, column: Column) -> String {
        format!(
            "ALTER TABLE {table} ADD COLUMN {}",
            column.add_column_expression()
        )
    }

    pub fn drop_column(table: &str, column: &str) -> String {
        format!("ALTER TABLE {table} DROP COLUMN {column}")
    }

    pub fn add_table(table: Table) -> String {
        format!(
            "CREATE TABLE {} ({})",
            table.name,
            table
                .columns
                .iter()
                .map(Column::add_column_expression)
                .my_join(", ")
        )
    }

    pub fn drop_table(table: &str) -> String {
        format!("DROP TABLE {table}")
    }

    pub fn query_schema_exists(schema: &str) -> String {
        format!(
            "
            SELECT EXISTS(
                SELECT 
                    1 
                FROM 
                    information_schema.schemata 
                WHERE schema_name = '{schema}'
            )"
        )
    }

    /// Query all tables and their columns of a schema.
    ///
    /// Returns the columns `table_name`, `column_name`, `data_type`, `is_nullable`.
    pub fn query_tables_and_columns(schema: &str) -> String {
        format!(
            "
            SELECT 
                table_name, 
                column_name, 
                data_type, 
                is_nullable
            FROM 
                information_schema.columns
            WHERE 
                table_schema = '{schema}'
            ORDER BY 
                table_name, 
                column_name"
        )
    }

    /// Query all tables and their constraints.
    ///
    /// Returns the columns `table_name`, `constraint_name`, `constraint_type`,
    /// `definition` (for `CHECK` constriants), `columns`
    /// (which is a list of all covered columns)
    /// and `ref_table` and `ref_columns` for `FOREIGN KEY` target columns.
    pub fn query_tables_and_constraints(schema: &str) -> String {
        format!(
            "
            SELECT
                tc.table_name,
                tc.constraint_name,
                tc.constraint_type,
                cc.check_clause AS definition,
                ARRAY_AGG(kcu.column_name ORDER BY kcu.ordinal_position) AS columns,
                fk.ref_table_name,
                fk.ref_columns
            FROM
                information_schema.table_constraints tc
            LEFT JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
            LEFT JOIN information_schema.check_constraints cc ON tc.constraint_name = cc.constraint_name AND tc.table_schema = cc.constraint_schema
            LEFT JOIN (
            SELECT
                kcu.table_name,
                kcu.constraint_name,
                kcu.table_schema,
                ARRAY_AGG(kcu.column_name ORDER BY kcu.position_in_unique_constraint) AS ref_columns,
                    rc.unique_constraint_name,
                    kcu2.table_name AS ref_table_name
                FROM
                    information_schema.referential_constraints rc
                JOIN information_schema.key_column_usage kcu ON rc.constraint_name = kcu.constraint_name AND rc.constraint_schema = kcu.table_schema
                JOIN information_schema.key_column_usage kcu2 ON rc.unique_constraint_name = kcu2.constraint_name AND rc.unique_constraint_schema = kcu2.table_schema
                GROUP BY
                    kcu.table_name,
                    kcu.constraint_name,
                    kcu.table_schema,
                    rc.unique_constraint_name,
                    kcu2.table_name
            ) fk ON tc.constraint_name = fk.constraint_name AND tc.table_schema = fk.table_schema
            WHERE
                tc.table_schema = '{schema}'
            GROUP BY
                tc.table_name, tc.constraint_name, tc.constraint_type, cc.check_clause, fk.ref_table_name, fk.ref_columns
            ORDER BY
                tc.table_name, tc.constraint_type"
        )
    }
}

impl TryFrom<&Row> for Constraint {
    type Error = MigrationError;

    fn try_from(row: &Row) -> Result<Self, Self::Error> {
        let table_name: String = row
            .try_get("table_name")
            .map_err(MigrationError::ParsingError)?;

        let constraint_name: String = row
            .try_get("constraint_name")
            .map_err(MigrationError::ParsingError)?;

        let constraint_type_raw: String = row
            .try_get("constraint_type")
            .map_err(MigrationError::ParsingError)?;

        let constraint_type = match constraint_type_raw.as_str() {
            "PRIMARY KEY" => ConstraintType::PrimaryKey {
                columns: row
                    .try_get("columns")
                    .map_err(MigrationError::ParsingError)?,
            },
            "UNIQUE" => ConstraintType::Unique {
                columns: row
                    .try_get("columns")
                    .map_err(MigrationError::ParsingError)?,
            },
            "FOREIGN KEY" => ConstraintType::ForeignKey {
                columns: row
                    .try_get("columns")
                    .map_err(MigrationError::ParsingError)?,
                foreign_table: row
                    .try_get("ref_table")
                    .map_err(MigrationError::ParsingError)?,
                foreign_columns: row
                    .try_get("ref_columns")
                    .map_err(MigrationError::ParsingError)?,
            },
            "CHECK" => ConstraintType::Check {
                definition: row
                    .try_get("definition")
                    .map_err(MigrationError::ParsingError)?,
            },
            unknown => return Err(MigrationError::UnexpectedValue(unknown.to_string())),
        };

        Ok(Constraint {
            constraint_name,
            table_name,
            constraint_type,
        })
    }
}

impl TryFrom<&Row> for Column {
    type Error = MigrationError;

    fn try_from(row: &Row) -> Result<Self, Self::Error> {
        let column_name: String = row
            .try_get("column_name")
            .map_err(MigrationError::ParsingError)?;
        let data_type: String = row
            .try_get("data_type")
            .map_err(MigrationError::ParsingError)?;
        let is_nullable_raw: String = row
            .try_get("is_nullable")
            .map_err(MigrationError::ParsingError)?;
        let is_nullable = match is_nullable_raw.as_str() {
            "YES" => true,
            "NO" => false,
            unknown => return Err(MigrationError::UnexpectedValue(unknown.to_string())),
        };

        Ok(Column {
            name: column_name,
            data_type,
            not_null: !is_nullable,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pool::Connection;

    #[tokio::test]
    async fn migrate() -> Result<(), Box<dyn std::error::Error>> {
        Connection::build("postgres://postgres:postgres@localhost:5432")
            .connect()
            .await
            .unwrap();
        Ok(())
    }
}
