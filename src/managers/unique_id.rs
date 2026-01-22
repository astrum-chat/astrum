use std::fmt::Display;

use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlError, ToSqlOutput, ValueRef},
};

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct UniqueId(String);

impl UniqueId {
    pub(crate) fn new() -> Self {
        Self(nanoid::nanoid!())
    }

    pub(crate) fn from_string(string: impl Into<String>) -> Self {
        Self(string.into())
    }
}

impl AsRef<UniqueId> for UniqueId {
    fn as_ref(&self) -> &UniqueId {
        &self
    }
}

impl Display for UniqueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ToSql for UniqueId {
    fn to_sql(&'_ self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.0.to_string().into())
    }
}

impl FromSql for UniqueId {
    fn column_result(value: ValueRef<'_>) -> Result<Self, FromSqlError> {
        match value {
            ValueRef::Text(text) => Ok(UniqueId(
                String::from_utf8(text.to_vec()).map_err(|_| FromSqlError::InvalidType)?,
            )),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}
