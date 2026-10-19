//! Application error type. Commands return `AppResult<T>`; errors reach the
//! frontend as `{ kind, message }` with a Russian, user-facing message.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Не найдено: {0}")]
    NotFound(String),
    #[error("Некорректные данные: {0}")]
    Invalid(String),
    #[error("Нет разрешения: {0}")]
    PermissionDenied(String),
    #[error("Ошибка базы данных: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Ошибка формата данных: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Invalid(_) => "invalid",
            Self::PermissionDenied(_) => "permission_denied",
            Self::Db(_) => "database",
            Self::Json(_) => "json",
            Self::Other(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let json = serde_json::to_value(AppError::NotFound("макет".into())).unwrap();
        assert_eq!(json["kind"], "not_found");
        assert_eq!(json["message"], "Не найдено: макет");
    }
}
