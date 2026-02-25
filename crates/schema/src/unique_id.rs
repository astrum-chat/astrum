use std::fmt::Display;

use notitia::{
    AsDatatypeKind, Datatype, DatatypeConversionError, DatatypeKind, DatatypeKindMetadata,
    InnerFieldType,
};

#[derive(Hash, PartialEq, Eq, Clone, Debug, Default)]
pub struct UniqueId(String);

impl UniqueId {
    pub fn new() -> Self {
        Self(nanoid::nanoid!())
    }

    pub fn from_string(string: impl Into<String>) -> Self {
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

impl AsDatatypeKind for UniqueId {
    fn as_datatype_kind() -> DatatypeKind {
        DatatypeKind::Text(DatatypeKindMetadata::default())
    }
}

impl Into<Datatype> for UniqueId {
    fn into(self) -> Datatype {
        Datatype::Text(self.0)
    }
}

impl TryFrom<Datatype> for UniqueId {
    type Error = DatatypeConversionError;

    fn try_from(d: Datatype) -> Result<Self, Self::Error> {
        String::try_from(d).map(UniqueId)
    }
}

impl InnerFieldType for UniqueId {
    type Inner = UniqueId;
}
