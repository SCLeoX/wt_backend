use std::ops::Deref;

use actix_web::{Either, HttpResponse, post, Responder, web};
use actix_web::dev::HttpServiceFactory;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::result::Error;
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{AppState, DbConnection};
use crate::api::common;
use crate::api::common::{APIResult, ErrorCode};
use crate::error::WTError;
use crate::models::User;
use crate::schema::{comments, mentions, users};

pub const TOKEN_LENGTH: usize = 32;
const MAX_USER_NAME_BYTES: usize = 64;
const MIN_USER_NAME_BYTES: usize = 3;
const MAX_EMAIL_BYTES: usize = 128;

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

pub fn is_token(token: &str) -> bool {
    token.chars().all(|ch| ch.is_ascii_alphanumeric()) && token.len() == TOKEN_LENGTH
}

pub fn get_user(connection: &DbConnection, token: &str) -> Result<Option<User>, Error> {
    if !is_token(token) {
        return Ok(None);
    }
    let user: Option<User> = users::table
        .filter(users::token.eq(token))
        .first(connection)
        .optional()?;
    Ok(user)
}

pub fn get_user_id(connection: &DbConnection, token: &str) -> Result<Option<i64>, Error> {
    if !is_token(token) {
        return Ok(None);
    }
    let user_id: Option<i64> = users::table
        .filter(users::token.eq(token))
        .select(users::id)
        .first(connection)
        .optional()?;
    Ok(user_id)
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

#[post("/init")]
async fn init_handler(state: web::Data<AppState>, query: web::Json<InitQuery>) -> Result<impl Responder, WTError> {
    let connection = state.db_pool.get()?;
    if let Some(user) = web::block(move || get_user(&connection, &query.token)).await? {
        let statement = mentions::table
            .inner_join(comments::table)
            .filter(mentions::timestamp.ge(user.last_checked_mentions_timestamp))
            .filter(mentions::mentioned_user_id.eq(user.id))
            .filter(comments::deleted.eq(false))
            .count();
        let connection = state.db_pool.get()?;
        let new_mentions: i64 = web::block(move || statement.get_result(&connection)).await?;
        Ok(HttpResponse::Ok().json(InitResponse {
            success: true,
            user_name: user.user_name,
            display_name: user.display_name,
            email: user.email,
            mentions: new_mentions,
        }))
    } else {
        Ok(common::error_response())
    }
}

#[derive(Deserialize)]
struct RegisterPayload {
    display_name: String,
    email: Option<String>,
}

#[derive(Serialize)]
struct RegisterResponse {
    token: String,
    user_name: String,
}

fn register<TCon: Deref<Target=DbConnection>>(
    connection: TCon,
    token: String,
    user_name: String,
    display_name: String,
    email: Option<String>,
    current_timestamp: i64
) -> Result<APIResult<RegisterResponse>, WTError> {
    if diesel::select(diesel::dsl::exists(users::table.filter(
        users::display_name.eq(&display_name)
            .or(users::user_name.eq(&user_name))))).get_result(&*connection)? {
        return Ok(APIResult::error(ErrorCode::NameDuplicated));
    }
    if let Some(user_email) = &email {
        if diesel::select(diesel::dsl::exists(users::table.filter(
            users::email.eq(user_email)))).get_result(&*connection)? {
            return Ok(APIResult::error(ErrorCode::EmailDuplicated));
        }
    }
    insert_into(users::table)
        .values((
            users::user_name.eq(&user_name),
            users::display_name.eq(&display_name),
            users::email.eq(&email),
            users::token.eq(&token),
            users::last_checked_mentions_timestamp.eq(current_timestamp)
        )).execute(&*connection)?;
    Ok(APIResult::success_return(RegisterResponse { token, user_name }))
}

#[post("/register")]
async fn register_handler(state: web::Data<AppState>, payload: web::Json<RegisterPayload>) -> Result<impl Responder, WTError> {
    if let Some(error_code) = validate_display_name(&payload.display_name) {
        return Ok(Either::A(common::error_response_with_code(error_code)));
    }
    if let Some(email) = &payload.email {
        if let Some(error_code) = validate_email(&email) {
            return Ok(Either::A(common::error_response_with_code(error_code)));
        }
    }
    let token: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(TOKEN_LENGTH)
        .collect();
    let user_name = payload.display_name.replace(' ', "_").to_ascii_lowercase();
    let connection = state.db_pool.get()?;
    let current_timestamp = common::get_current_timestamp();
    Ok(Either::B(web::block(move || register(
        connection,
        token,
        user_name,
        payload.0.display_name,
        payload.0.email,
        current_timestamp
    )).await?.into_responder()))
}

#[derive(Deserialize)]
struct UpdateProfilePayload {
    token: String,
    display_name: String,
    email: Option<String>,
}

fn update_profile<TCon: Deref<Target=DbConnection>>(connection: TCon, token: String, display_name: String, email: Option<String>) -> Result<APIResult, WTError> {
    let user = get_user(&connection, &token)?;
    if let Some(user) = user {
        if diesel::select(diesel::dsl::exists(users::table
            .filter(users::token.ne(&token))
            .filter(users::display_name.eq(&display_name))
        )).get_result(&*connection)? {
            return Ok(APIResult::error(ErrorCode::NameDuplicated));
        }
        if let Some(user_email) = &email {
            if diesel::select(diesel::dsl::exists(users::table
                .filter(users::token.ne(&token))
                .filter(users::email.eq(user_email))
            )).get_result(&*connection)? {
                return Ok(APIResult::error(ErrorCode::EmailDuplicated));
            }
        }
        diesel::update(&user)
            .set((
                users::email.eq(&email),
                users::display_name.eq(&display_name)
            )).execute(&*connection)?;
        Ok(APIResult::success())
    } else {
        Ok(APIResult::forbidden())
    }
}

#[post("/updateProfile")]
async fn update_profile_handler(state: web::Data<AppState>, payload: web::Json<UpdateProfilePayload>) -> Result<impl Responder, WTError> {
    if let Some(error_code) = validate_display_name(&payload.display_name) {
        return Ok(Either::A(common::error_response_with_code(error_code)));
    }
    if let Some(email) = &payload.email {
        if let Some(error_code) = validate_email(email) {
            return Ok(Either::A(common::error_response_with_code(error_code)));
        }
    }
    let connection = state.db_pool.get()?;
    Ok(Either::B(web::block(move || update_profile(
        connection,
        payload.0.token,
        payload.0.display_name,
        payload.0.email,
    )).await?.into_responder()))
}

pub fn get_service() -> impl HttpServiceFactory {
    web::scope("/user")
        .service(init_handler)
        .service(register_handler)
        .service(update_profile_handler)
}
