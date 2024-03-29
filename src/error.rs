use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

use actix_web::http::{header, StatusCode};
use actix_web::{error, HttpResponse};

#[derive(Debug)]
pub enum WTError {
    InternalError(Box<dyn Error + Send>),
}

impl<T: Error + Send + 'static> From<T> for WTError {
    fn from(error: T) -> Self {
        WTError::InternalError(Box::new(error))
    }
}

impl Display for WTError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "An internal error occurred.")
        // match self {
        //     WTError::InternalError(_) => write!(f, "An internal error occurred."),
        // }
    }
}

impl error::ResponseError for WTError {
    fn status_code(&self) -> StatusCode {
        match *self {
            WTError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    fn error_response(&self) -> HttpResponse {
        eprintln!("{:?}", self);
        HttpResponse::build(self.status_code())
            .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
            .body(self.to_string())
    }
}
