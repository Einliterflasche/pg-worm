use std::{marker::PhantomData, ops::Deref};

use tokio_postgres::{types::ToSql, Row};

use crate::{Error, Filter};

pub(crate) struct PgTable {
    table_name: String,
    columns: Vec<PgColumn>,
}

pub(crate) struct PgColumn {
    column_name: String,
    data_type: String,
    is_nullable: bool,
    is_identity: bool,
    is_generated: bool,
}

/// A wrapper around the [`Column`] struct which includes
/// the rust type of the field.
///
/// For each field of a [`pg_worm::Model`] a `TypedColumn` is automatically
/// generated.
///
/// A `TypedColumn` can be used to access information about
/// the column and create `Filter`s regarding this column.
///
/// # Example
///
/// ```
/// use pg_worm::Model;
///
/// #[derive(Model)]
/// struct Foo {
///     baz: i64
/// }
///
/// assert_eq!(Foo::baz.column_name(), "baz");
///
/// ```
///
#[derive(Clone, Copy, Debug)]
pub struct TypedColumn<T: ToSql + Sync> {
    column: Column,
    rs_type: PhantomData<T>,
}

/// This type represents a column.
///  
/// It can be used to retrieve information about the column.
///
/// It is mostly seen in it's wrapped form [`TypedColumn`].
#[derive(Copy, Clone, Debug)]
pub struct Column {
    column_name: &'static str,
    table_name: &'static str,
    nullable: bool,
    unique: bool,
    primary_key: bool,
    generated: bool,
}

impl TryFrom<&Row> for PgColumn {
    type Error = Error;

    fn try_from(value: &Row) -> Result<Self, Self::Error> {
        Ok(Self {
            column_name: value.try_get("column_name")?,
            is_nullable: value.try_get("is_nullable")?,
            data_type: value.try_get("data_type")?,
            is_generated: value.try_get("is_generated")?,
            is_identity: value.try_get("is_identity")?,
        })
    }
}

macro_rules! impl_prop_typed_col {
    ($($prop:ident),+) => {
        $(
            pub const fn $prop(mut self) -> TypedColumn<T> {
                self.column.$prop = true;
                self
            }
        )*
    };
}

macro_rules! impl_prop_col {
    ($($prop:ident),+) => {
        $(
            pub const fn $prop(&self) -> bool {
                self.$prop
            }
        )*
    };
}

impl<T: ToSql + Sync + Send + 'static> TypedColumn<T> {
    pub const fn new(table_name: &'static str, column_name: &'static str) -> TypedColumn<T> {
        TypedColumn {
            column: Column::new(table_name, column_name),
            rs_type: PhantomData::<T>,
        }
    }

    /// Get the column's name
    pub const fn name(&self) -> &'static str {
        self.column.column_name
    }

    impl_prop_typed_col!(nullable, unique, primary_key, generated);

    /// Check whether the columns value is equal to `value`.
    ///
    /// Translates to `WHERE <column_name> = <value>`.
    pub fn eq(&self, value: impl Into<T>) -> Filter {
        Filter::new(
            format!("{} = $1", self.column.full_name()),
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
            format!("{} IN ({placeholders})", self.column.full_name()),
            vals,
        )
    }

    pub fn is_null(&self) -> Filter {
        Filter::new(format!("{} IS NULL", self.column.full_name()), Vec::new())
    }
}

impl TypedColumn<String> {
    /// Query for values which are `LIKE val`.
    pub fn like(&self, val: impl Into<String>) -> Filter {
        let val: String = val.into();

        Filter::new(format!("{} LIKE $1", self.full_name()), vec![Box::new(val)])
    }
}

impl<T: ToSql + Sync> Deref for TypedColumn<T> {
    type Target = Column;

    fn deref(&self) -> &Self::Target {
        &self.column
    }
}

impl Column {
    pub const fn new(table_name: &'static str, column_name: &'static str) -> Column {
        Column {
            column_name,
            table_name,
            nullable: false,
            unique: false,
            primary_key: false,
            generated: false,
        }
    }

    impl_prop_col!(unique, nullable, primary_key, generated);

    /// Get the column name.
    pub const fn column_name(&self) -> &'static str {
        self.column_name
    }

    /// Get the name of the table this column
    /// is part of.
    pub const fn table_name(&self) -> &'static str {
        self.table_name
    }

    /// Get the full name of the column.
    ///
    /// # Example
    ///
    /// ```
    /// use pg_worm::Model;
    ///
    /// #[derive(Model)]
    /// struct Foo {
    ///     baz: String
    /// }
    /// assert_eq!(Foo::baz.full_name(), "foo.baz");
    /// ```
    #[inline]
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.table_name, self.column_name)
    }
}
