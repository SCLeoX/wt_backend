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

use crate::database::{get_db_executor, Init, ListChapterRecent, ListChaptersAll, RecordVisit, Register, RegisterResult, TimeFrame, SendComment, GetUser, AddMentions, UpdateProfile, UpdateProfileResult, GetChapterComments, GetRecentComments, GetRecentMentionedComments};
use crate::models::{Comment, User};
use percent_encoding::NON_ALPHANUMERIC;
use crate::dark_colors::DARK_COLORS;

pub const TOKEN_LENGTH: usize = 32;
pub const MAX_USER_NAME_BYTES: usize = 64;
pub const MIN_USER_NAME_BYTES: usize = 3;
pub const MAX_EMAIL_BYTES: usize = 128;
pub const MAX_COMMENT_BYTES: usize = 4096;
pub const MIN_COMMENT_BYTES: usize = 1;
pub const MAX_PAGE_NAME_BYTES: usize = 1024;
pub const MIN_PAGE_NAME_BYTES: usize = 1;
pub const MAX_MENTIONS_PER_COMMENT: usize = 5;

#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u8)]
enum ErrorCode {
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

pub mod schema;
mod models;
mod database;
mod error;
mod dark_colors;

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

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
}
fn error_response() -> HttpResponse {
    HttpResponse::Ok().json(ErrorResponse { success: false })
}
#[derive(Serialize)]
struct ErrorResponseWithCode {
    success: bool,
    code: ErrorCode,
}
fn error_response_with_code(code: ErrorCode) -> HttpResponse {
    HttpResponse::Ok().json(ErrorResponseWithCode { success: false, code })
}

#[derive(Deserialize)]
struct InitQuery {
    token: String,
}
#[derive(Serialize)]
struct InitResponse {
    success: bool,
    user_name: String,
    display_name: String,
    email: Option<String>,
    mentions: i64,
}
async fn init_handler(state: web::Data<AppState>, query: web::Json<InitQuery>) -> Result<impl Responder, WTError> {
    if let Some(result) = state.db.send(Init { token: query.0.token }).await?? {
        Ok(HttpResponse::Ok().json(InitResponse {
            success: true,
            user_name: result.user_name,
            display_name: result.display_name,
            email: result.email,
            mentions: result.mentions,
        }))
    } else {
        Ok(error_response())
    }
}

#[derive(Serialize)]
struct SimpleSuccessResponse {
    success: bool,
}

fn simple_success() -> HttpResponse {
    HttpResponse::Ok().json(SimpleSuccessResponse { success: true })
}

fn is_page_name_valid(page_name: &str) -> bool {
    page_name.len() <= MAX_PAGE_NAME_BYTES && page_name.len() >= MIN_PAGE_NAME_BYTES
}
fn validate_display_name(user_name: &str) -> Option<ErrorCode> {
    if user_name.len() > MAX_USER_NAME_BYTES {
        return Some(ErrorCode::NameTooLong);
    }
    if user_name.len() < MIN_USER_NAME_BYTES {
        return Some(ErrorCode::NameTooShort);
    }
    None
}
fn validate_email(email: &str) -> Option<ErrorCode> {
    if email.len() > MAX_EMAIL_BYTES {
        return Some(ErrorCode::EmailTooLong);
    }
    lazy_static! {
            static ref EMAIL_REGEX: Regex = Regex::new("^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+.[a-zA-Z0-9-.]+$").unwrap();
        }
    if !EMAIL_REGEX.is_match(email) {
        return Some(ErrorCode::EmailInvalid);
    }
    None
}
#[derive(Deserialize)]
struct RegisterPayload {
    display_name: String,
    email: Option<String>,
}
#[derive(Serialize)]
struct RegisterResponse {
    success: bool,
    token: String,
    user_name: String,
}
async fn register_handler(state: web::Data<AppState>, payload: web::Json<RegisterPayload>) -> Result<impl Responder, WTError> {
    if let Some(error_code) = validate_display_name(&payload.display_name) {
        return Ok(error_response_with_code(error_code));
    }
    if let Some(email) = &payload.email {
        if let Some(error_code) = validate_email(&email) {
            return Ok(error_response_with_code(error_code));
        }
    }

    let token: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(TOKEN_LENGTH)
        .collect();
    let user_name = payload.display_name.replace(' ', "_").to_ascii_lowercase();
    match state.db.send(Register {
        user_name: user_name.clone(),
        display_name: payload.0.display_name,
        email: payload.0.email,
        token: token.clone(),
    }).await?? {
        RegisterResult::Ok => {
            Ok(HttpResponse::Ok().json(RegisterResponse { success: true, token, user_name }))
        }
        RegisterResult::DuplicatedName => {
            Ok(error_response_with_code(ErrorCode::NameDuplicated))
        }
        RegisterResult::DuplicatedEmail => {
            Ok(error_response_with_code(ErrorCode::EmailDuplicated ))
        }
    }
}

#[derive(Deserialize)]
struct UpdateProfilePayload {
    token: String,
    display_name: String,
    email: Option<String>,
}
async fn update_profile_handler(state: web::Data<AppState>, payload: web::Json<UpdateProfilePayload>) -> Result<impl Responder, WTError> {
    if let Some(error_code) = validate_display_name(&payload.display_name) {
        return Ok(error_response_with_code(error_code));
    }
    if let Some(email) = &payload.email {
        if let Some(error_code) = validate_email(email) {
            return Ok(error_response_with_code(error_code));
        }
    }
    match state.db.send(UpdateProfile {
        token: payload.0.token,
        display_name: payload.0.display_name,
        email: payload.0.email,
    }).await?? {
        UpdateProfileResult::Ok => {
            Ok(simple_success())
        }
        UpdateProfileResult::InvalidToken => {
            Ok(error_response_with_code(ErrorCode::TokenInvalid))
        }
        UpdateProfileResult::DuplicatedName => {
            Ok(error_response_with_code(ErrorCode::NameDuplicated))
        }
        UpdateProfileResult::DuplicatedEmail => {
            Ok(error_response_with_code(ErrorCode::EmailDuplicated ))
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
        return Ok(Either::A(error_response_with_code(ErrorCode::CommentTooLong)));
    }
    if payload.content.len() < MIN_COMMENT_BYTES {
        return Ok(Either::A(error_response_with_code(ErrorCode::CommentTooShort)));
    }
    if (payload.token.len() != TOKEN_LENGTH) || (!is_page_name_valid(&payload.relative_path)) {
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
        let mut mentioned = Vec::new();
        for (mentions_count, capture) in MENTION_REGEX.captures_iter(&payload.content).enumerate() {
            mentioned.push(capture[1].to_owned());
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
        if !mentioned.is_empty() {
            state.db.send(AddMentions {
                comment_id: send_comment_result.comment_id,
                mentioned,
                current_timestamp,
            }).await??;
        }
        Ok(Either::A(simple_success()))
    } else {
        Ok(Either::B(HttpResponse::Forbidden()))
    }
}

#[derive(Serialize)]
struct SingleUserResponse {
    avatar_url: String,
    user_name: String,
    display_name: String,
}
#[derive(Serialize)]
struct SingleCommentResponse {
    body: String,
    create_timestamp: i64,
    update_timestamp: i64,
    chapter_relative_path: String,
    id: i64,
    user: SingleUserResponse,
}
fn get_user_avatar_url(user: &User) -> String {
    let display_name_encoded = percent_encoding::utf8_percent_encode(&user.display_name, NON_ALPHANUMERIC).to_string();
    let color_hash = seahash::hash(user.user_name.as_bytes());
    let color = DARK_COLORS[(color_hash % (DARK_COLORS.len() as u64)) as usize];
    if let Some(email) = &user.email {
        // Due to weird interaction between gravatar and ui-avatars, we have to encode display_name again
        let display_name_encoded = percent_encoding::utf8_percent_encode(&display_name_encoded, NON_ALPHANUMERIC).to_string();
        format!("https://www.gravatar.com/avatar/{:x}?d=https%3A%2F%2Fui-avatars.com%2Fapi%2F{}%2F128%2F{}%2Fffffff", md5::compute(email), display_name_encoded, color)
    } else {
        format!("https://ui-avatars.com/api/{}/128/{}/ffffff", display_name_encoded, color)
    }
}
fn convert_comment_user_tuples_to_response(tuples: Vec<(String, Comment, User)>) -> Vec<SingleCommentResponse> {
    tuples.into_iter().map(|(chapter_relative_path, comment, user)| SingleCommentResponse {
        body: comment.content,
        create_timestamp: comment.create_timestamp,
        update_timestamp: comment.update_timestamp,
        chapter_relative_path,
        id: comment.id,
        user: SingleUserResponse {
            avatar_url: get_user_avatar_url(&user),
            user_name: user.user_name,
            display_name: user.display_name,
        },
    }).collect()
}

#[derive(Deserialize)]
struct GetChapterCommentsQuery {
    relative_path: String,
}
async fn get_chapter_comments_handler(state: web::Data<AppState>, query: web::Query<GetChapterCommentsQuery>) -> Result<Either<impl Responder, impl Responder>, WTError> {
    if !is_page_name_valid(&query.relative_path) {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
    let comments = state.db.send(GetChapterComments {
        chapter_relative_path: query.0.relative_path,
    }).await??;
    Ok(Either::A(HttpResponse::Ok().json(convert_comment_user_tuples_to_response(comments))))
}

async fn get_recent_comments_handler(state: web::Data<AppState>) -> Result<impl Responder, WTError> {
    let comments = state.db.send(GetRecentComments()).await??;
    Ok(HttpResponse::Ok().json(convert_comment_user_tuples_to_response(comments)))
}

#[derive(Deserialize)]
struct GetRecentMentionedCommentsPayload {
    token: String,
}
async fn get_recent_mentioned_comments_handler(state: web::Data<AppState>, payload: web::Json<GetRecentMentionedCommentsPayload>) -> Result<impl Responder, WTError> {
    let comments = state.db.send(GetRecentMentionedComments {
        token: payload.0.token
    }).await??;
    Ok(HttpResponse::Ok().json(convert_comment_user_tuples_to_response(comments)))
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
            .allowed_methods(vec!["GET", "POST"])
            .allowed_header("Content-Type")
            .max_age(3600);
        App::new()
            .data(AppState {
                db: db_addr.clone(),
            })
            .wrap(cors)
            .service(
                web::resource("/count")
                    .app_data(String::configure(|cfg| {
                        cfg.limit(MAX_PAGE_NAME_BYTES)
                    }))
                    .route(web::post().to(count_handler))
            )
            .route("/stats/chapters/all", web::get().to(chapter_all_handler))
            .route("/stats/chapters/recent", web::get().to(chapter_recent_handler))
            .route("/init", web::post().to(init_handler))
            .route("/register", web::post().to(register_handler))
            .route("/sendComment", web::post().to(send_comment_handler))
            .route("/updateProfile", web::post().to(update_profile_handler))
            .route("/getChapterComments", web::get().to(get_chapter_comments_handler))
            .route("/getRecentComments", web::get().to(get_recent_comments_handler))
            .route("/getRecentMentionedComments", web::post().to(get_recent_mentioned_comments_handler))
    })
        .bind("127.0.0.1:8088")?
        .run()
        .await
}
