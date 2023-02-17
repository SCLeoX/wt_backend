// @generated automatically by Diesel CLI.

diesel::table! {
    chapters (id) {
        id -> Int4,
        relative_path -> Varchar,
        visit_count -> Int8,
    }
}

diesel::table! {
    comments (id) {
        id -> Int8,
        chapter_id -> Int4,
        user_id -> Int8,
        content -> Varchar,
        deleted -> Bool,
        create_timestamp -> Int8,
        update_timestamp -> Int8,
    }
}

diesel::table! {
    mentions (id) {
        id -> Int8,
        from_comment_id -> Int8,
        mentioned_user_id -> Int8,
        timestamp -> Int8,
    }
}

diesel::table! {
    users (id) {
        id -> Int8,
        token -> Bpchar,
        email -> Nullable<Varchar>,
        user_name -> Varchar,
        display_name -> Varchar,
        disabled -> Bool,
        last_checked_mentions_timestamp -> Int8,
    }
}

diesel::table! {
    visits (id) {
        id -> Int8,
        chapter_id -> Int4,
        timestamp -> Int8,
    }
}

diesel::table! {
    wtcup_2020_votes (id) {
        id -> Int8,
        user_id -> Int8,
        chapter_vote_id -> Int2,
        rating -> Int2,
    }
}

diesel::table! {
    wtcup_2021_votes (id) {
        id -> Int8,
        user_id -> Int8,
        chapter_vote_id -> Int2,
        rating -> Int2,
    }
}

diesel::table! {
    wtcup_2022_votes (id) {
        id -> Int8,
        user_id -> Int8,
        chapter_vote_id -> Int2,
        rating -> Int2,
    }
}

diesel::joinable!(mentions -> comments (from_comment_id));
diesel::joinable!(mentions -> users (mentioned_user_id));
diesel::joinable!(wtcup_2021_votes -> users (user_id));
diesel::joinable!(wtcup_2022_votes -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    chapters,
    comments,
    mentions,
    users,
    visits,
    wtcup_2020_votes,
    wtcup_2021_votes,
    wtcup_2022_votes,
);
