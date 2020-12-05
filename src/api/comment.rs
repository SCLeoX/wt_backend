use std::ops::Deref;

use actix_web::{Either, get, HttpResponse, post, Responder, web};
use actix_web::dev::HttpServiceFactory;
use diesel::insert_into;
use diesel::prelude::*;
use percent_encoding::NON_ALPHANUMERIC;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{AppState, DbConnection};
use crate::api::common::ErrorCode;
use crate::api::user::get_user;
use crate::dark_colors::DARK_COLORS;
use crate::error::WTError;
use crate::models::{Comment, User};
use crate::schema::chapters;
use crate::schema::comments;
use crate::schema::mentions;
use crate::schema::users;

use super::common;

pub const MAX_COMMENT_BYTES: usize = 4096;
pub const MIN_COMMENT_BYTES: usize = 1;
pub const MAX_MENTIONS_PER_COMMENT: usize = 5;
const RECENT_COMMENTS_AMOUNT: i64 = 50;

#[derive(Deserialize)]
struct SendCommentPayload {
    token: String,
    relative_path: String,
    content: String,
}

fn send<TCon: Deref<Target=DbConnection>>(
    connection: TCon,
    token: String,
    relative_path: String,
    content: String,
    current_timestamp: i64,
    mentioned: Vec<String>,
) -> Result<bool, WTError> {
    let user = get_user(&connection, &token)?;
    if let Some(user) = user {
        connection.transaction::<bool, WTError, _>(|| {
            let chapter = common::get_chapter(&*connection, &relative_path)?;
            let comment_id: i64 = insert_into(comments::table)
                .values((
                    comments::chapter_id.eq(chapter.id),
                    comments::user_id.eq(user.id),
                    comments::content.eq(&content),
                    comments::deleted.eq(false),
                    comments::create_timestamp.eq(current_timestamp),
                    comments::update_timestamp.eq(current_timestamp),
                ))
                .returning(comments::id)
                .get_result(&*connection)?;
            if !mentioned.is_empty() {
                let user_ids: Vec<i64> = users::table
                    .select(users::id)
                    .filter(users::user_name.eq_any(mentioned))
                    .get_results(&*connection)?;
                insert_into(mentions::table)
                    .values(user_ids.into_iter().map(|user_id| (
                        mentions::from_comment_id.eq(comment_id),
                        mentions::mentioned_user_id.eq(user_id),
                        mentions::timestamp.eq(current_timestamp)
                    )).collect::<Vec<_>>())
                    .execute(&*connection)?;
            }
            Ok(true)
        })
    } else {
        Ok(false)
    }
}

#[post("/send")]
async fn send_handler(state: web::Data<AppState>, payload: web::Json<SendCommentPayload>) -> Result<impl Responder, WTError> {
    let payload = payload.0;
    if payload.content.len() > MAX_COMMENT_BYTES {
        return Ok(Either::A(common::error_response_with_code(ErrorCode::CommentTooLong)));
    }
    if payload.content.len() < MIN_COMMENT_BYTES {
        return Ok(Either::A(common::error_response_with_code(ErrorCode::CommentTooShort)));
    }
    if (!super::user::is_token(&payload.token)) || (!common::is_page_name(&payload.relative_path)) {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
    lazy_static! {
        static ref MENTION_REGEX: Regex = Regex::new("@(\\S+)(?:\\s|$)").unwrap();
    }
    let current_timestamp = common::get_current_timestamp();
    let mut mentioned = Vec::new();
    for (mentions_count, capture) in MENTION_REGEX.captures_iter(&payload.content).enumerate() {
        mentioned.push(capture[1].to_owned());
        if mentions_count >= MAX_MENTIONS_PER_COMMENT {
            break;
        }
    }
    mentioned.sort();
    mentioned.dedup();
    let connection = state.db_pool.get()?;
    if web::block(move || send(
        connection,
        payload.token,
        payload.relative_path,
        payload.content,
        current_timestamp,
        mentioned,
    )).await? {
        Ok(Either::A(common::simple_success()))
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
    relative_path: String,
    id: i64,
    user: SingleUserResponse,
}

fn get_user_avatar_url(user: &User) -> String {
    let display_name_encoded = percent_encoding::utf8_percent_encode(&user.display_name, NON_ALPHANUMERIC).to_string();
    let color = DARK_COLORS[(seahash::hash(user.user_name.as_bytes()) % (DARK_COLORS.len() as u64)) as usize];
    if let Some(email) = &user.email {
        // Due to weird interaction between gravatar and ui-avatars, we have to encode display_name again
        // However, since all special characters except % are gone, we can do a simple replace from % to %25
        let display_name_encoded = display_name_encoded.replace('%', "%25");
        format!("https://www.gravatar.com/avatar/{:x}?d=https%3A%2F%2Fui-avatars.com%2Fapi%2F{}%2F128%2F{}%2Fffffff", md5::compute(email), display_name_encoded, color)
    } else {
        format!("https://ui-avatars.com/api/{}/128/{}/ffffff", display_name_encoded, color)
    }
}

#[derive(Queryable)]
struct SingleCommentQueryResult {
    relative_path: String,
    comment: Comment,
    user: User,
}

type CommentQueryResults = Vec<SingleCommentQueryResult>;

fn convert_comment_query_results_to_response(comment_query_result: CommentQueryResults) -> Vec<SingleCommentResponse> {
    comment_query_result.into_iter().map(|SingleCommentQueryResult { relative_path, comment, user }| SingleCommentResponse {
        body: comment.content,
        create_timestamp: comment.create_timestamp,
        update_timestamp: comment.update_timestamp,
        relative_path,
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

fn get_chapter<TCon: Deref<Target=DbConnection>>(connection: TCon, relative_path: String) -> Result<CommentQueryResults, WTError> {
    Ok(chapters::table
        .inner_join(comments::table.inner_join(users::table))
        .select((chapters::relative_path, comments::table::all_columns(), users::table::all_columns()))
        .filter(chapters::relative_path.eq(&relative_path))
        .filter(comments::deleted.eq(false))
        .order_by(comments::id.desc())
        .load(&*connection)?)
}

#[get("/getChapter")]
async fn get_chapter_handler(state: web::Data<AppState>, query: web::Query<GetChapterCommentsQuery>) -> Result<Either<impl Responder, impl Responder>, WTError> {
    if !common::is_page_name(&query.relative_path) {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
    let connection = state.db_pool.get()?;
    let results = web::block(move || get_chapter(connection, query.0.relative_path)).await?;
    Ok(Either::A(HttpResponse::Ok().json(convert_comment_query_results_to_response(results))))
}

fn get_recent<TCon: Deref<Target=DbConnection>>(connection: TCon) -> Result<CommentQueryResults, WTError> {
    Ok(comments::table
        .inner_join(users::table)
        .inner_join(chapters::table)
        .select((chapters::relative_path, comments::table::all_columns(), users::table::all_columns()))
        .filter(comments::deleted.eq(false))
        .order_by(comments::id.desc())
        .limit(RECENT_COMMENTS_AMOUNT)
        .load(&*connection)?)
}

#[get("/getRecent")]
async fn get_recent_comments_handler(state: web::Data<AppState>) -> Result<impl Responder, WTError> {
    let connection = state.db_pool.get()?;
    let results = web::block(move || get_recent(connection)).await?;
    Ok(HttpResponse::Ok().json(convert_comment_query_results_to_response(results)))
}

#[derive(Deserialize)]
struct GetRecentMentionedCommentsPayload {
    token: String,
}

fn get_recent_mentioned<TCon: Deref<Target=DbConnection>>(connection: TCon, token: String, current_timestamp: i64) -> Result<CommentQueryResults, WTError> {
    let user = get_user(&connection, &token)?;
    if let Some(user) = user {
        diesel::update(&user)
            .set(users::last_checked_mentions_timestamp.eq(current_timestamp))
            .execute(&*connection)?;
        Ok(mentions::table
            .inner_join(comments::table.inner_join(chapters::table).inner_join(users::table))
            .select((chapters::relative_path, comments::table::all_columns(), users::table::all_columns()))
            .filter(comments::deleted.eq(false))
            .filter(mentions::mentioned_user_id.eq(user.id))
            .order_by(comments::id.desc())
            .limit(RECENT_COMMENTS_AMOUNT)
            .load(&*connection)?)
    } else {
        Ok(vec![])
    }
}

#[post("/getRecentMentioned")]
async fn get_recent_mentioned_comments_handler(state: web::Data<AppState>, payload: web::Json<GetRecentMentionedCommentsPayload>) -> Result<impl Responder, WTError> {
    if !super::user::is_token(&payload.token) {
        return Ok(Either::B(HttpResponse::Forbidden()));
    }
    let connection = state.db_pool.get()?;
    let current_timestamp = common::get_current_timestamp();
    let results = web::block(move || get_recent_mentioned(connection, payload.0.token, current_timestamp)).await?;
    Ok(Either::A(HttpResponse::Ok().json(convert_comment_query_results_to_response(results))))
}

pub fn get_service() -> impl HttpServiceFactory {
    web::scope("/comment")
        .service(send_handler)
        .service(get_chapter_handler)
        .service(get_recent_comments_handler)
        .service(get_recent_mentioned_comments_handler)
}
