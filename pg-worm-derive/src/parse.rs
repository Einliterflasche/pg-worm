use darling::{ast::Data, FromDeriveInput, FromField};
use postgres_types::Type;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

#[derive(Clone, FromField)]
#[darling(attributes(column))]
pub struct ModelField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    #[darling(default)]
    primary_key: bool,
    #[darling(default)]
    unique: bool,
    dtype: Option<String>,
    column_name: Option<String>,
    #[darling(default)]
    auto: bool,
}

#[derive(FromDeriveInput)]
#[darling(attributes(table), supports(struct_named))]
pub struct ModelInput {
    ident: syn::Ident,
    data: Data<(), ModelField>,
    table_name: Option<String>,
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

    pub fn n_fields(&self) -> usize {
        match &self.data {
            Data::Struct(fields) => fields.fields.len(),
            _ => panic!("only named struct supported"),
        }
    }

    pub fn fields(&self) -> impl Iterator<Item = &ModelField> {
        match &self.data {
            Data::Struct(fields) => fields.fields.iter(),
            _ => panic!("only named struct supported"),
        }
    }

    pub fn insert_fields(&self) -> impl Iterator<Item = &ModelField> {
        self.fields().filter(|f| !f.auto_generated())
    }

    pub fn table_creation_sql(&self) -> String {
        format!(
            "DROP TABLE IF EXISTS {} CASCADE; CREATE TABLE {} ({})",
            self.table_name(),
            self.table_name(),
            self.fields()
                .map(|f| f.column_creation_sql())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl ModelField {
    pub fn auto_generated(&self) -> bool {
        self.auto
            || self.primary_key
            || self.dtype.as_ref().is_some()
                && self
                    .dtype
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("serial")
    }

    pub fn ty(&self) -> &syn::Type {
        &self.ty
    }

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
    pub fn pg_datatype(&self) -> Type {
        if let Some(dtype) = &self.dtype {
            let ty = match dtype.to_lowercase().as_str() {
                "bool" | "boolean" => Type::BOOL,
                "text" => Type::TEXT,
                "int" | "integer" | "int4" => Type::INT4,
                "bigint" | "int8" => Type::INT8,
                "smallint" | "int2" => Type::INT2,
                "real" => Type::FLOAT4,
                "double precision" => Type::FLOAT8,
                "bigserial" => Type::INT8,
                _ => panic!("couldn't find postgres type `{}`", dtype),
            };

            return ty;
        }

        match self.ty() {
            syn::Type::Path(type_path) => match type_path
                    .path
                    .segments
                    .first().unwrap()
                    .ident.to_string().as_str() {
                        "String" => Type::TEXT,
                        "i64" => Type::INT8,
                        "f32" => Type::FLOAT4,
                        "f64" => Type::FLOAT8,
                        "bool" => Type::BOOL,
                        _ => todo!(
                            "cannot guess postgres type for field {:?}, please provide via attribute: `#[column(dtype = 'DataType']`", 
                            self.ident().to_string()
                        )
                    },
            syn::Type::Reference(_) => panic!("field {:?} may not be reference", self.ident().to_string()),
            _ => todo!(
                "cannot guess postgres type for field {:?}, please provide via attribute: `#[column(dtype = 'DataType']`", 
                self.ident().to_string()
            )
        }
    }

    /// Get the SQL representing the column needed
    /// for creating a table.
    ///
    /// # Example
    ///
    pub fn column_creation_sql(&self) -> String {
        if self.primary_key && self.unique {
            panic!(
                "primary keys are unique, remove unnecessary `unique` on {:?}",
                self.ident().to_string()
            )
        }

        // The list of "args" for the sql statement.
        // Includes at least the column name and datatype.
        let mut args = vec![self.column_name(), self.pg_datatype().to_string()];

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
        arg!(self.auto, "GENERATED ALWAYS AS IDENTITY");
        arg!(self.unique, "UNIQUE");

        // Join the args, seperated by a space and return them
        args.join(" ")
    }

    pub fn insert_arg_type(&self) -> TokenStream {
        let ty = self.ty().to_token_stream();
        if ty.to_string() == "String" {
            return quote!(impl Into<String> + pg_worm::pg::types::ToSql + Sync);
        }
        ty
    }
}
