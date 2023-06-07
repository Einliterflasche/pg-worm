use std::ops::{BitAnd, BitOr, Not};

use tokio_postgres::types::ToSql;

/// Struct for filtering your queries. Maps
/// to the `WHERE` clause of your query.
///
/// _These are automatically generated by operations
/// like `MyModel::my_field.eq(5)`. **You are not supposed to
/// construct them manually.**_
///
/// Stores the statement
/// and arguments. The statement should include placeholders
/// in the form of `$1`, `$2` and so on.
pub struct Filter {
    args: Vec<Box<dyn ToSql + Sync + Send>>,
    stmt: String,
}

impl Filter {
    pub(crate) fn new(stmt: impl Into<String>, args: Vec<Box<dyn ToSql + Sync + Send>>) -> Filter {
        Filter {
            stmt: stmt.into(),
            args,
        }
    }

    /// Creates a new filter which doesn't filter anything.
    pub fn all() -> Filter {
        Filter::new("", Vec::new())
    }

    /// Access the filter's raw sql statement.
    ///
    #[inline]
    pub fn _stmt(&self) -> &str {
        self.stmt.as_str()
    }

    #[inline]
    pub fn _args(&self) -> &Vec<Box<dyn ToSql + Sync + Send>> {
        &self.args
    }

    #[inline]
    pub fn args(self) -> Vec<Box<dyn ToSql + Sync + Send>> {
        self.args
    }

    fn combine_with_sep(mut f1: Filter, f2: Filter, sep: &str) -> Filter {
        if f1._stmt().trim().is_empty() {
            return f2;
        }

        if f2._stmt().trim().is_empty() {
            return f1;
        }

        let mut left_stmt = f1.stmt + sep;
        let mut right_stmt = f2.stmt;

        while let Some(i) = right_stmt.find('$') {
            // Compute number of digits of the current placeholder number
            let mut digs: usize = 0usize;
            loop {
                let slice = &right_stmt[i + 1 + digs..];
                if let Some(c) = slice.chars().next() {
                    if c.is_numeric() {
                        digs += 1;
                        continue;
                    }
                }
                break;
            }

            // Parse the number
            let num: usize = right_stmt[i + 1..=i + digs].parse().unwrap();

            // Add everything before the number to the left stmt
            // assert!(curr <= i, "!{curr} <= {i}");
            left_stmt.push_str(&right_stmt[..=i]);
            // Add the new number to the left statement
            left_stmt.push_str(&format!("{}", num + f1.args.len()));
            // Repeat for the rest of the placeholders
            let new_start = i + digs + 1;
            right_stmt = right_stmt[new_start..].to_string();
        }

        // Add rest of the string
        left_stmt += &right_stmt;

        f1.args.extend(f2.args);

        Filter::new(left_stmt, f1.args)
    }

    #[inline]
    pub fn to_sql(&self) -> String {
        if self.stmt.trim().is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.stmt)
        }
    }
}

impl BitAnd for Filter {
    type Output = Filter;

    fn bitand(self, rhs: Self) -> Self::Output {
        Filter::combine_with_sep(self, rhs, " AND ")
    }
}

impl BitOr for Filter {
    type Output = Filter;

    fn bitor(self, rhs: Self) -> Self::Output {
        Filter::combine_with_sep(self, rhs, " OR ")
    }
}

impl Not for Filter {
    type Output = Filter;

    fn not(self) -> Self::Output {
        Filter::new(format!("NOT ({})", self.stmt), self.args)
    }
}
