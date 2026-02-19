use chrono::NaiveDateTime;
use notitia::{
    AsDatatypeKind, Datatype, DatatypeConversionError, DatatypeKind, DatatypeKindMetadata,
    InnerFieldType,
};

const FORMAT: &str = "%Y-%m-%d %H:%M:%S%.f";
const FORMAT_NO_FRAC: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DbDateTime(pub NaiveDateTime);

impl DbDateTime {
    pub fn now() -> Self {
        Self(chrono::Utc::now().naive_utc())
    }
}

impl AsDatatypeKind for DbDateTime {
    fn as_datatype_kind() -> DatatypeKind {
        DatatypeKind::Text(DatatypeKindMetadata::default())
    }
}

impl Into<Datatype> for DbDateTime {
    fn into(self) -> Datatype {
        Datatype::Text(self.0.format(FORMAT).to_string())
    }
}

impl TryFrom<Datatype> for DbDateTime {
    type Error = DatatypeConversionError;

    fn try_from(d: Datatype) -> Result<Self, Self::Error> {
        let s = String::try_from(d)?;
        NaiveDateTime::parse_from_str(&s, FORMAT)
            .or_else(|_| NaiveDateTime::parse_from_str(&s, FORMAT_NO_FRAC))
            .map(DbDateTime)
            .map_err(|_| DatatypeConversionError::TypeMismatch {
                expected: "DateTime",
                got: "Text",
            })
    }
}

impl InnerFieldType for DbDateTime {
    type Inner = DbDateTime;
}
