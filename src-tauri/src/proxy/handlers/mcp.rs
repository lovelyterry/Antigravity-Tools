use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn handle_web_search_prime() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

pub async fn handle_web_reader() -> Response {
    StatusCode::NOT_FOUND.into_response()
}
