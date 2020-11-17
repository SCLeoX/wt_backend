#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;

use actix::Addr;
use actix_cors::Cors;
use actix_web::{App, FromRequest, HttpResponse, HttpServer, Responder, web, Either};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use database::DbExecutor;
use error::WTError;

use crate::database::{get_db_executor, Init, ListChapterRecent, ListChaptersAll, RecordVisit, Register, RegisterResult, TimeFrame, SendComment, GetUser, AddMentions};

pub const TOKEN_LENGTH: usize = 32;
pub const MAX_USER_NAME_BYTES: usize = 64;
pub const MAX_EMAIL_BYTES: usize = 128;
pub const MAX_COMMENT_BYTES: usize = 4096;
pub const MAX_PAGE_NAME_BYTES: usize = 1024;
pub const MAX_MENTIONS_PER_COMMENT: usize = 5;

#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum ErrorCode {
    UserNameDuplicated = 1,
    EmailDuplicated = 2,
    UserNameTooLong = 3,
    EmailTooLong = 4,
    EmailInvalid = 5,
    CommentTooLong = 6,
}

pub mod schema;
mod models;
mod database;
mod error;

async fn count_handler(state: web::Data<AppState>, content: String) -> Result<impl Responder, WTError> {
    state.db.send(RecordVisit { relative_path: content }).await??;
    Ok(HttpResponse::Ok().body("<3"))
}

#[derive(Deserialize)]
struct ChapterAllQuery {
    page: i32,
}
async fn chapter_all_handler(state: web::Data<AppState>, query: web::Query<ChapterAllQuery>) -> Result<impl Responder, WTError> {
    let result = state.db.send(ListChaptersAll { page: query.page }).await??;
    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
struct ChapterRecentQuery {
    page: i32,
    time_frame: TimeFrame,
}
async fn chapter_recent_handler(state: web::Data<AppState>, query: web::Query<ChapterRecentQuery>) -> Result<impl Responder, WTError> {
    let result = state.db.send(ListChapterRecent { page: query.page, time_frame: query.time_frame }).await??;
    Ok(HttpResponse::Ok().json(result))
}

#[derive(Deserialize)]
struct InitQuery {
    token: String,
    since: i64,
}
async fn init_handler(state: web::Data<AppState>, query: web::Query<InitQuery>) -> Result<impl Responder, WTError> {
    if let Some(result) = state.db.send(Init { token: query.0.token, since: query.0.since }).await?? {
        Ok(HttpResponse::Ok().json(result))
    } else {
        Ok(HttpResponse::Forbidden().body("Invalid token"))
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    code: ErrorCode,
}

fn error_response(code: ErrorCode) -> HttpResponse {
    HttpResponse::Forbidden().json(ErrorResponse { success: false, code })
}

#[derive(Serialize)]
struct SimpleSuccessResponse {
    success: bool,
}

fn simple_success() -> HttpResponse {
    HttpResponse::Ok().json(SimpleSuccessResponse { success: true })
}

#[derive(Deserialize)]
struct RegisterPayload {
    user_name: String,
    email: Option<String>,
}
#[derive(Serialize)]
struct RegisterResponse {
    success: bool,
    token: String,
}
async fn register_handler(state: web::Data<AppState>, payload: web::Json<RegisterPayload>) -> Result<impl Responder, WTError> {
    if payload.user_name.len() > MAX_USER_NAME_BYTES {
        return Ok(error_response(ErrorCode::UserNameTooLong));
    }
    if let Some(email) = &payload.email {
        if email.len() > MAX_EMAIL_BYTES {
            return Ok(error_response(ErrorCode::EmailTooLong ));
        }
        lazy_static! {
            static ref EMAIL_REGEX: Regex = Regex::new("^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+.[a-zA-Z0-9-.]+$").unwrap();
        }
        if !EMAIL_REGEX.is_match(email) {
            return Ok(error_response(ErrorCode::EmailInvalid));
        }
    }

    let token: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(TOKEN_LENGTH)
        .collect();
    let display_name = payload.user_name.replace(' ', "_");
    match state.db.send(Register {
        user_name: payload.0.user_name,
        display_name,
        email: payload.0.email,
        token: token.clone(),
    }).await?? {
        RegisterResult::Ok => {
            Ok(HttpResponse::Ok().json(RegisterResponse { success: true, token }))
        }
        RegisterResult::DuplicatedUserName => {
            Ok(error_response(ErrorCode::UserNameDuplicated))
        }
        RegisterResult::DuplicatedEmail => {
            Ok(error_response(ErrorCode::EmailDuplicated ))
        }
    }
}

#[derive(Deserialize)]
struct SendCommentPayload {
    token: String,
    relative_path: String,
    content: String,
}
async fn send_comment_handler(state: web::Data<AppState>, payload: web::Json<SendCommentPayload>) -> Result<Either<impl Responder, impl Responder>, WTError> {
    let payload = payload.0;
    if payload.content.len() > MAX_COMMENT_BYTES {
        return Ok(Either::A(error_response(ErrorCode::CommentTooLong)));
    }
    if (payload.token.len() != TOKEN_LENGTH) || (payload.relative_path.len() > MAX_PAGE_NAME_BYTES) {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
    let user = state.db.send(GetUser {
        token: payload.token,
    }).await??;
    if let Some(user) = user {
        lazy_static! {
            static ref MENTION_REGEX: Regex = Regex::new("@(\\S+)(?:\\s|$)").unwrap();
        }
        let current_timestamp = database::get_current_timestamp();
        // Get mentions first, but insert them later
        let mut mentions_count = 0;
        let mut mentioned = Vec::with_capacity(5);
        for capture in MENTION_REGEX.captures_iter(&payload.content) {
            mentioned.push(capture[1].to_owned());
            mentions_count += 1;
            if mentions_count >= MAX_MENTIONS_PER_COMMENT {
                break;
            }
        }
        mentioned.sort();
        mentioned.dedup();
        let send_comment_result = state.db.send(SendComment {
            user_id: user.id,
            relative_path: payload.relative_path,
            content: payload.content,
            current_timestamp,
        }).await??;
        if mentioned.len() >= 1 {
            state.db.send(AddMentions {
                comment_id: send_comment_result.comment_id,
                mentioned,
                current_timestamp,
            }).await??;
        }
        Ok(Either::A(simple_success()))
    } else {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
}

struct AppState {
    db: Addr<DbExecutor>
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let db_addr = get_db_executor();
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://127.0.0.1:2333")
            .allowed_origin("http://localhost:2333")
            .allowed_origin("https://wt.tepis.me")
            .allowed_origin("https://wt.bgme.me")
            .allowed_origin("https://rbq.desi")
            .allowed_origin("https://wt.makai.city")
            .allowed_origin("https://wt.0w0.bid")
            .max_age(3600);
        App::new()
            .wrap(cors)
            .data(AppState {
                db: db_addr.clone(),
            })
            .service(
                web::resource("/count")
                    .app_data(String::configure(|cfg| {
                        cfg.limit(MAX_PAGE_NAME_BYTES)
                    }))
                    .route(web::post().to(count_handler))
            )
            .route(
                "/stats/chapters/all",
                web::get().to(chapter_all_handler),
            )
            .route(
                "/stats/chapters/recent",
                web::get().to(chapter_recent_handler),
            )
            .route(
                "/init",
                web::get().to(init_handler),
            )
            .route(
                "/register",
                web::post().to(register_handler),
            )
            .route(
                "/sendComment",
                web::post().to(send_comment_handler),
            )
    })
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
