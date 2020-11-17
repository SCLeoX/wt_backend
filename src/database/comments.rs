use actix::{Handler, Message};
use diesel::result::Error;
use diesel::prelude::*;
use diesel::{PgConnection};
use serde::Serialize;

use super::db_executor::DbExecutor;
use super::common;
use crate::models::User;

pub fn get_user(connection: &PgConnection, token_value: &str) -> Result<Option<User>, Error> {
    if token_value.len() != 32 {
        return Ok(None)
    }
    use crate::schema::users::dsl::*;
    let user: Option<User> = users
        .filter(token.eq(token_value))
        .first(connection)
        .optional()?;
    Ok(user)
}

pub fn get_user_by_user_name(connection: &PgConnection, user_name_value: &str) -> Result<Option<User>, Error> {
    use crate::schema::users::dsl::*;
    let user: Option<User> = users
        .filter(user_name.eq(user_name_value))
        .first(connection)
        .optional()?;
    Ok(user)
}

pub struct GetUser {
    pub token: String,
}
impl Message for GetUser {
    type Result = Result<Option<User>, Error>;
}
impl Handler<GetUser> for DbExecutor {
    type Result = Result<Option<User>, Error>;
    fn handle(&mut self, msg: GetUser, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        get_user(connection, &msg.token)
    }
}

pub struct Init {
    pub token: String,
    pub since: i64,
}
#[derive(Serialize)]
pub struct InitResult {
    pub user_name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub mentions: i64,
}
impl Message for Init {
    type Result = Result<Option<InitResult>, Error>;
}
impl Handler<Init> for DbExecutor {
    type Result = Result<Option<InitResult>, Error>;
    fn handle(&mut self, msg: Init, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        if let Some(user) = get_user(connection, &msg.token)? {
            use crate::schema::mentions::dsl::*;
            let new_mentions: i64 = mentions
                .filter(timestamp.ge(msg.since))
                .filter(mentioned_user_id.eq(user.id))
                .count()
                .get_result(connection)?;
            Ok(Some(InitResult {
                user_name: user.user_name,
                display_name: user.display_name,
                email: user.email,
                mentions: new_mentions,
            }))
        } else {
            Ok(None)
        }
    }
}


pub struct Register {
    pub user_name: String,
    pub display_name: String,
    pub email: Option<String>,
    pub token: String,
}
pub enum RegisterResult {
    Ok,
    DuplicatedUserName,
    DuplicatedEmail,
}
impl Message for Register {
    type Result = Result<RegisterResult, Error>;
}
impl Handler<Register> for DbExecutor {
    type Result = Result<RegisterResult, Error>;
    fn handle(&mut self, msg: Register, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        use crate::schema::users::dsl::*;
        use diesel::dsl::*;
        if select(exists(users.filter(
            display_name.eq(&msg.display_name)
                .or(user_name.eq(&msg.user_name))
        ))).get_result(connection)? {
            return Ok(RegisterResult::DuplicatedUserName);
        }
        if let Some(user_email) = &msg.email {
            if select(exists(users.filter(email.eq(user_email)))).get_result(connection)? {
                return Ok(RegisterResult::DuplicatedEmail);
            }
        }
        insert_into(users)
            .values((
                user_name.eq(msg.user_name),
                display_name.eq(msg.display_name),
                email.eq(msg.email),
                token.eq(msg.token),
            ))
            .execute(connection)?;
        return Ok(RegisterResult::Ok);
    }
}

pub struct SendComment {
    pub user_id: i64,
    pub relative_path: String,
    pub content: String,
    pub current_timestamp: i64,
}
pub struct SendCommentResult {
    pub comment_id: i64,
}
impl Message for SendComment {
    type Result = Result<SendCommentResult, Error>;
}
impl Handler<SendComment> for DbExecutor {
    type Result = Result<SendCommentResult, Error>;
    fn handle(&mut self, msg: SendComment, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        let chapter = common::get_chapter(connection, &msg.relative_path)?;
        use crate::schema::comments::dsl::*;
        use diesel::dsl::*;
        let comment_id = insert_into(comments)
            .values((
                chapter_id.eq(chapter.id),
                user_id.eq(msg.user_id),
                content.eq(msg.content),
                deleted.eq(false),
                create_timestamp.eq(msg.current_timestamp),
                update_timestamp.eq(msg.current_timestamp),
            ))
            .returning(id)
            .get_result(connection)?;
        Ok(SendCommentResult { comment_id })
    }
}

pub struct AddMentions {
    pub comment_id: i64,
    pub mentioned: Vec<String>,
    pub current_timestamp: i64,
}
impl Message for AddMentions {
    type Result = Result<(), Error>;
}
impl Handler<AddMentions> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: AddMentions, _: &mut Self::Context) -> Self::Result {
        let connection = &self.0;
        for mentioned_user_name in msg.mentioned {
            let user = get_user_by_user_name(connection, &mentioned_user_name)?;
            if let Some(user) = user {
                use crate::schema::mentions::dsl::*;
                use diesel::dsl::*;
                insert_into(mentions)
                    .values((
                        from_comment_id.eq(msg.comment_id),
                        mentioned_user_id.eq(user.id),
                        timestamp.eq(msg.current_timestamp)
                    ))
                    .execute(connection)?;
            }
        }
        Ok(())
    }
}