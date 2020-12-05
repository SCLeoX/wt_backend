use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{Either, HttpResponse, Responder};
use diesel::insert_into;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::result::Error;
use serde::Serialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::models::Chapter;

pub const MAX_PAGE_NAME_BYTES: usize = 1024;
pub const MIN_PAGE_NAME_BYTES: usize = 1;

#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ErrorCode {
    NameDuplicated = 1,
    EmailDuplicated = 2,
    NameTooLong = 3,
    EmailTooLong = 4,
    EmailInvalid = 5,
    CommentTooLong = 6,
    TokenInvalid = 7,
    NameTooShort = 8,
    CommentTooShort = 9,
}

pub fn get_chapter(connection: &PgConnection, relative_path_value: &str) -> Result<Chapter, Error> {
    use crate::schema::chapters::dsl::*;
    let chapter: Option<Chapter> = chapters
        .filter(relative_path.eq(relative_path_value))
        .first(connection)
        .optional()?;
    if let Some(chapter) = chapter {
        Ok(chapter)
    } else {
        let row: Chapter = insert_into(chapters)
            .values(relative_path.eq(relative_path_value))
            .get_result(connection)?;
        Ok(row)
    }
}

pub fn get_current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis().try_into().expect("Hello future")
}

pub fn is_page_name(page_name: &str) -> bool {
    page_name.len() <= MAX_PAGE_NAME_BYTES && page_name.len() >= MIN_PAGE_NAME_BYTES
}

#[derive(Serialize)]
struct SimpleSuccessResponse {
    success: bool,
}

pub fn simple_success() -> HttpResponse {
    HttpResponse::Ok().json(SimpleSuccessResponse { success: true })
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
}

pub fn error_response() -> HttpResponse {
    HttpResponse::Ok().json(ErrorResponse { success: false })
}

#[derive(Serialize)]
struct ErrorResponseWithCode {
    success: bool,
    code: ErrorCode,
}

pub fn error_response_with_code(code: ErrorCode) -> HttpResponse {
    HttpResponse::Ok().json(ErrorResponseWithCode { success: false, code })
}

#[derive(Serialize)]
pub struct Empty {}

#[derive(Serialize)]
pub enum APIResult<T: Serialize = Empty> {
    Success(T),
    Error(ErrorCode),
    Forbidden,
}

impl<T: Serialize> APIResult<T> {
    pub fn success_return(value: T) -> Self {
        APIResult::Success(value)
    }
    pub fn error(code: ErrorCode) -> Self {
        APIResult::Error(code)
    }
    pub fn forbidden() -> Self {
        APIResult::Forbidden
    }
    pub fn into_responder(self) -> impl Responder {
        match self {
            APIResult::Success(value) => {
                #[derive(Serialize)]
                struct SerializeHelper<T: Serialize> {
                    success: bool,

                    #[serde(flatten)]
                    value: T,
                }
                Either::A(HttpResponse::Ok().json(SerializeHelper { success: true, value }))
            }
            APIResult::Error(code) => {
                #[derive(Serialize)]
                struct SerializeHelper {
                    success: bool,
                    code: ErrorCode,
                }
                Either::A(HttpResponse::Ok().json(SerializeHelper { success: false, code }))
            }
            APIResult::Forbidden => {
                Either::B(HttpResponse::Forbidden())
            }
        }
    }
}

impl APIResult<Empty> {
    pub fn success() -> Self {
        APIResult::Success(Empty {})
    }
}
