use crate::schema::chapters;
use crate::schema::comments;
use crate::schema::mentions;
use crate::schema::users;
use crate::schema::visits;
use crate::schema::wtcup_2020_votes;

#[derive(Identifiable, Queryable)]
pub struct Chapter {
    pub id: i32,
    pub relative_path: String,
    pub visit_count: i64,
}

#[derive(Identifiable, Queryable)]
pub struct Visit {
    pub id: i64,
    pub chapter_id: i32,
    pub timestamp: i64,
}

#[derive(Identifiable, Queryable)]
pub struct Comment {
    pub id: i64,
    pub chapter_id: i32,
    pub user_id: i64,
    pub content: String,
    pub deleted: bool,
    pub create_timestamp: i64,
    pub update_timestamp: i64,
}

joinable!(comments -> users (user_id));
joinable!(comments -> chapters (chapter_id));

#[derive(Identifiable, Queryable)]
pub struct Mention {
    pub id: i64,
    pub from_comment_id: i64,
    pub mentioned_user_id: i64,
    pub timestamp: i64,
}

#[derive(Identifiable, Queryable, AsChangeset)]
#[table_name = "users"]
#[changeset_options(treat_none_as_null = "true")]
pub struct User {
    pub id: i64,
    pub token: String,
    pub email: Option<String>,
    pub user_name: String,
    pub display_name: String,
    pub disabled: bool,
    pub last_checked_mentions_timestamp: i64,
}

#[derive(Identifiable, Queryable)]
#[table_name = "wtcup_2020_votes"]
pub struct WTCup2020Vote {
    pub id: i64,
    pub user_id: i64,
    pub chapter_vote_id: i16,
    pub rating: i16,
}

joinable!(wtcup_2020_votes -> users (user_id));
