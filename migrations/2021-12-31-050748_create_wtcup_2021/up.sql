CREATE TABLE wtcup_2021_votes(
    id bigserial NOT NULL,
    user_id bigint NOT NULL,
    chapter_vote_id smallint NOT NULL,
    rating smallint NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT wtcup_2021_votes_user_chapter_vote_unique UNIQUE (user_id, chapter_vote_id),
    CONSTRAINT wtcup_2021_votes_user_id_fkey FOREIGN KEY (user_id)
        REFERENCES users (id)
);
