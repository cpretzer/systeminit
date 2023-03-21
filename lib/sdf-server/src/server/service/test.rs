use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use dal::{TransactionsError, UserError, WorkspaceError};
use names::{Generator, Name};
use thiserror::Error;

mod signup;
mod signup_and_login;

#[derive(Debug, Error)]
pub enum TestError {
    #[error(transparent)]
    Nats(#[from] si_data_nats::NatsError),
    #[error(transparent)]
    Pg(#[from] si_data_pg::PgError),
    #[error(transparent)]
    ContextTransaction(#[from] TransactionsError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

pub type TestResult<T> = std::result::Result<T, TestError>;

impl IntoResponse for TestError {
    fn into_response(self) -> Response {
        let (status, error_message) = (StatusCode::INTERNAL_SERVER_ERROR, self.to_string());

        let body = Json(serde_json::json!({
            "error": {
                "message": error_message,
                "code": 42,
                "statusCode": status.as_u16(),
            },
        }));

        (status, body).into_response()
    }
}

pub(crate) fn generate_fake_name() -> String {
    Generator::with_naming(Name::Numbered)
        .next()
        .expect("ran out of random names")
}

pub fn routes() -> Router {
    Router::new()
        .route("/fixtures/signup", post(signup::signup))
        .route(
            "/fixtures/signup_and_login",
            post(signup_and_login::signup_and_login),
        )
}