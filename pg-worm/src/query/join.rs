use super::{DynCol, ToQuery};

/// A struct representing SQL joins.
pub struct Join {
    column: &'static DynCol,
    on_column: &'static DynCol,
    join_type: JoinType,
}

/// The different types of SQL joins.
pub enum JoinType {
    Inner,
    Outer,
    Left,
    Right,
}

impl Join {
    pub const fn new(c1: &'static DynCol, c2: &'static DynCol, ty: JoinType) -> Join {
        Self {
            column: c1,
            on_column: c2,
            join_type: ty,
        }
    }
}

impl ToQuery for Join {
    fn to_sql(&self) -> String {
        let join_type: &'static str = match self.join_type {
            JoinType::Inner => "INNER",
            JoinType::Outer => "OUTER",
            JoinType::Left => "LEFT",
            JoinType::Right => "RIGHT",
        };

        format!(
            "{join_type} JOIN {0} ON {1}.{2} = {0}.{3}",
            self.on_column.table_name(),
            self.column.table_name(),
            self.column.column_name(),
            self.on_column.column_name()
        )
    }
}

impl PartialEq for Join {
    fn eq(&self, other: &Self) -> bool {
        self.to_sql().eq(&other.to_sql())
    }
}
