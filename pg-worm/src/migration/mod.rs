//! This module contains the logic needed to create automatic migrations.

#[derive(Debug, PartialEq, Eq, Hash)]
struct Table {
    name: &'static str,
    columns: Vec<Column>,
    primary_key: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct Column {
    table: &'static str,
    name: &'static str,
    data_type: &'static str,
    constraints: Vec<ColumnConstraint>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ColumnConstraint {
    Unique,
    UniqueNamed(&'static str),
    NotNull,
    PrimaryKey,
    ForeignKey(&'static str, &'static str),
    ForeignKeyNamed(&'static str, &'static str, &'static str),
    RawCheck(&'static str),
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum TableConstraint {
    PrimaryKey(Vec<&'static str>),
    ForeignKey(Vec<(&'static str, &'static str, &'static str)>),
    Unique(Vec<&'static str>),
    RawCheck(&'static str),
}

impl Table {
    fn new(name: &'static str) -> Self {
        let columns: Vec<Column> = Vec::new();
        let primary_key = Vec::new();

        Table {
            name,
            columns,
            primary_key,
        }
    }

    fn column(mut self, col: Column) -> Table {
        self.columns.push(col);

        self
    }
}

impl Column {
    fn new(table: &'static str, name: &'static str, data_type: &'static str) -> Self {
        Column {
            table,
            name,
            data_type,
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

    fn unique_named(mut self, name: &'static str) -> Self {
        self.constraints.push(ColumnConstraint::UniqueNamed(name));

        self
    }

    fn primary_key(mut self, name: &'static str) -> Self {
        self.constraints.push(ColumnConstraint::PrimaryKey);

        self
    }

    fn foreign_key(mut self, table_name: &'static str, column_name: &'static str) -> Self {
        self.constraints
            .push(ColumnConstraint::ForeignKey(table_name, column_name));

        self
    }
}

impl ToString for ColumnConstraint {
    fn to_string(&self) -> String {
        use ColumnConstraint as C;

        match self {
            C::Unique => "UNIQUE".into(),
            C::UniqueNamed(name) => format!("CONSTRAINT {} UNIQUE", name),
            C::NotNull => "NOT NULL".into(),
            C::PrimaryKey => "PRIMARY KEY".into(),
            C::ForeignKey(table, column) => format!("REFERENCES {} ({})", table, column),
            C::ForeignKeyNamed(name, table, column) => {
                format!("CONSTRAINT {} REFERENCES {} ({})", name, table, column)
            }
            C::RawCheck(raw) => (*raw).into(),
        }
    }
}

fn foo() {
    let table_name = "books";
    let table = Table::new(table_name)
        .column(Column::new(table_name, "id", "BIGINT"))
        .column(Column::new(table_name, "title", "TEXT"));
}
