use darling::{FromDeriveInput, FromField, ast::Data};
use syn::Ident;

#[derive(Clone, FromField)]
#[darling(attributes(column))]
pub struct ModelField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    /// Doesn't do anything as of yet.
    #[darling(default)]
    no: bool,
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    unique: bool,
    dtype: Option<String>,
    column_name: Option<String>,
}

#[derive(FromDeriveInput)]
#[darling(
    attributes(table),
    supports(struct_named)
)]
pub struct ModelInput {
    ident: syn::Ident,
    data: Data<(), ModelField>,
    table_name: Option<String>
}

impl ModelInput {
    pub fn table_name(&self) -> String {
        if let Some(table_name) = &self.table_name {
            return table_name.clone();
        }

        self.ident.to_string().to_lowercase()
    }

    pub const fn ident(&self) -> &Ident {
        &self.ident
    }

    pub fn fields(&self) -> impl Iterator<Item = &ModelField> {
        match &self.data {
            Data::Struct(fields) => 
                fields.fields
                    .iter(),
                    // Skip fields marked with `column(no)`
                    // .filter(|f| !f.no),
            _ => panic!("only named struct supported")
        }
    }

    pub fn get_create_sql(&self) -> String {
        format!(
            "CREATE TABLE IF NOT EXISTS {} ({})",
            self.table_name(),
            self.fields()
                .map(|f| f.get_create_sql())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl ModelField {
    /// Get the field's identifier.
    pub fn ident(&self) -> Ident {
        self.ident
            .clone()
            .expect("struct {} should only contain named fields")
    }

    /// Ge the column's name.
    pub fn column_name(&self) -> String {
        if let Some(column_name) = &self.column_name {
            return column_name.clone();
        }

        self.ident().to_string().to_lowercase()
    }

    /// Get the column's PostgreSQL datatype.
    pub fn get_datatype(&self) -> String {
        if let Some(dtype) = &self.dtype {
            return dtype.clone();
        }

        todo!(
            "cannot guess postgres type for field {:?}, please provide via attribute: `#[column(dtype = 'DataType']`", 
            self.ident().to_string()
        )
    }

    /// Get the SQL representing the column needed
    /// for creating a table.
    /// 
    /// # Example
    /// 
    pub fn get_create_sql(&self) -> String {
        
        // The list of "args" for the sql statement.
        // Includes at least the column name and datatype.
        let mut args = vec![
            self.column_name(),
            self.get_datatype()
        ];

        // This macro allows adding an arg to the list
        // under a condition.
        macro_rules! arg {
            ($cond:expr, $sql:literal) => {
                if $cond {
                    args.push($sql.to_string());
                }
            };
        }

        // Add possible args
        arg!(self.primary_key, "PRIMARY KEY");
        arg!(self.unique, "UNIQUE");

        // Join the args, seperated by a space and return them
        args.join(" ")
    }
}
