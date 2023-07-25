use std::{marker::PhantomData, ops::{Deref, Not}};

use tokio_postgres::types::ToSql;

use crate::query::Where;

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
/// use pg_worm::prelude::*;
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
    /// The name of this column.
    pub column_name: &'static str,
    /// The name of the table this columnn belongs to.
    pub table_name: &'static str,
    nullable: bool,
    unique: bool,
    primary_key: bool,
    generated: bool,
}

macro_rules! impl_prop_typed_col {
    ($($prop:ident),+) => {
        $(
            /// Set this property so `true`.
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
            /// Returns this propertie's value.
            pub const fn $prop(&self) -> bool {
                self.$prop
            }
        )*
    };
}

impl<T: ToSql + Sync + Send + 'static> TypedColumn<T> {
    /// Creates anew `TypedColumn<T>`.
    pub const fn new(table_name: &'static str, column_name: &'static str) -> TypedColumn<T> {
        TypedColumn {
            column: Column::new(table_name, column_name),
            rs_type: PhantomData::<T>,
        }
    }

    impl_prop_typed_col!(nullable, unique, primary_key, generated);

    /// Returns a [`Where`] clause which checks whether
    /// this column is equal to some value.
    pub fn eq<'a>(&self, other: &'a T) -> Where<'a> {
        Where::new(
            format!("{}.{} = ?", self.table_name, self.column_name),
            vec![other],
        )
    }
}

impl<T: ToSql + Sync + Send + 'static + PartialOrd> TypedColumn<T> {
    /// Check whether this column's value is **g**reater **t**han some
    /// other value.
    pub fn gt<'a>(&self, other: &'a T) -> Where<'a> {
        Where::new(
            format!("{}.{} > ?", self.table_name, self.column_name),
            vec![other],
        )
    }

    /// Check whether this colum's value is **g**reater **t**han or **e**qual
    /// to another value.
    pub fn gte<'a>(&self, other: &'a T) -> Where<'a> {
        Where::new(
            format!("{}.{} >= ?", self.table_name, self.column_name),
            vec![other],
        )
    }

    /// Check whether this column's value is **l**ess **t**han some
    /// other value.
    pub fn lt<'a>(&self, other: &'a T) -> Where<'a> {
        Where::new(
            format!("{}.{} < ?", self.table_name, self.column_name),
            vec![other],
        )
    }

    /// Check whether this colum's value is **l**ess **t**han or **e**qual
    /// to another value.
    pub fn lte<'a>(&self, other: &'a T) -> Where<'a> {
        Where::new(
            format!("{}.{} <= ?", self.table_name, self.column_name),
            vec![other],
        )
    }
}

impl<'a, T: ToSql + Sync + 'a> TypedColumn<Option<T>> {
    /// Check whether this column's value is `NULL`.
    pub fn null(&self) -> Where<'a> {
        Where::new(
            format!("{}.{} IS NULL", self.table_name, self.column_name), 
            vec![]
        )
    }

    /// Check whether this column's value is `NOT NULL`
    pub fn not_noll(&self) -> Where<'a> {
        self.null().not()
    }
} 

impl<'a, T: ToSql + Sync + 'a> TypedColumn<Vec<T>> {
    /// Check whether this column's array contains some value.
    pub fn contains(&self, value: &'a T) -> Where<'a> {
        Where::new(
            format!("? = ANY({}.{})", self.table_name, self.column_name),
            vec![value]
        )
    }

    /// Check whether this column's array does `NOT` contain some value.
    pub fn contains_not(&self, value: &'a T) -> Where<'a> {
        self.contains(value).not()
    }

    /// Check whether this column's array contains any value of 
    /// another array.
    pub fn contains_any(&self, values: &'a Vec<&'a T>) -> Where<'a> {
        Where::new(
            format!("{}.{} && ?", self.table_name, self.column_name),
            vec![values]
        )
    }

    /// Check whether this column's array contains all values of
    /// another array.
    pub fn contains_all(&self, values: &'a Vec<&'a T>) -> Where<'a> {
        Where::new(
            format!("{}.{} @> ?", self.table_name, self.column_name), 
            vec![values]
        )
    }

    /// Check whether this column's array does not overlap
    /// with another array, i.e. contains none of its values.
    pub fn contains_none(&self, values: &'a Vec<&'a T>) -> Where<'a> {
        self.contains_any(values).not()
    }
}

impl<T: ToSql + Sync> Deref for TypedColumn<T> {
    type Target = Column;

    fn deref(&self) -> &Self::Target {
        &self.column
    }
}

impl Column {
    /// Creates a new column
    const fn new(table_name: &'static str, column_name: &'static str) -> Column {
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
    /// use pg_worm::prelude::*;
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

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use crate::{
        prelude::*,
        query::{PushChunk, Where},
    };

    impl<'a> Where<'a> {
        /// This is a convieniance function for testing purposes.
        fn to_stmt(&mut self) -> String {
            let mut q = Query::<u64>::default();
            self.push_to_buffer(&mut q);

            q.0
        }
    }

    #[derive(Model)]
    struct Book {
        id: i64,
        title: String,
        pages: Vec<String>
    }

    #[test]
    fn equals() {
        assert_eq!(Book::title.eq(&"ABC".into()).to_stmt(), "book.title = ?")
    }

    #[test]
    fn greater_than() {
        assert_eq!(Book::id.gt(&1).to_stmt(), "book.id > ?");
    }

    #[test]
    fn greater_than_equals() {
        assert_eq!(Book::id.gte(&1).to_stmt(), "book.id >= ?");
    }

    #[test]
    fn less_than() {
        assert_eq!(Book::id.lt(&1).to_stmt(), "book.id < ?")
    }

    #[test]
    fn less_than_equals() {
        assert_eq!(Book::id.lte(&1).to_stmt(), "book.id <= ?")
    }

    #[test]
    fn complete_query() {
        let q = Book::select()
            .where_(Book::title.eq(&"The Communist Manifesto".into()))
            .where_(Book::pages.contains(&"You have nothing to lose but your chains!".into()))
            .where_(Book::id.gt(&3))
            .to_query().0;
        assert_eq!(q, "SELECT book.id, book.title, book.pages FROM book WHERE (book.title = $1) AND ($2 = ANY(book.pages)) AND (book.id > $3)");
    }
}
