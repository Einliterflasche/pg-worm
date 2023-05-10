use tokio_postgres::Row;

pub use pg_worm_derive::Model;

/// This trait allows comfortable querying.
///
/// # Usage
///
/// ```
/// use pg_worm::Model;
///
/// #[derive(Model)]
/// struct Book {
///     id: i64,
///     title: String
/// }
/// ```
pub trait Model<T> {
    ///
    fn from_row(row: &Row) -> Result<T, tokio_postgres::Error>;
}

#[cfg(test)]
mod tests {
    use crate::Model;

    #[derive(Model)]
    struct Person {
        id: i64,
        name: String,
    }
}
