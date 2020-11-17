CREATE TABLE public.users(
    id bigserial NOT NULL,
    token char(32) NOT NULL,
    email varchar(255),
    user_name varchar(255) NOT NULL,
    display_name varchar(255) NOT NULL,
    disabled bool NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id),
    CONSTRAINT email_unique UNIQUE (email),
    CONSTRAINT user_name_unique UNIQUE (user_name),
    CONSTRAINT display_name_unique UNIQUE (display_name)
);

CREATE TABLE public.comments(
    id bigserial NOT NULL,
    chapter_id integer NOT NULL,
    user_id bigint NOT NULL,
    content varchar(4096) NOT NULL,
    deleted bool NOT NULL DEFAULT FALSE,
    create_timestamp bigint NOT NULL,
    update_timestamp bigint NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT chapter_id_fkey FOREIGN KEY (chapter_id)
        REFERENCES public.chapters (id),
    CONSTRAINT user_id_fkey FOREIGN KEY  (user_id)
        REFERENCES public.users (id)
);

CREATE TABLE public.mentions(
    id bigserial NOT NULL,
    from_comment_id bigint NOT NULL,
    mentioned_user_id bigint NOT NULL,
    "timestamp" bigint NOT NULL,
    PRIMARY KEY (id),
    CONSTRAINT from_comment_id_fkey FOREIGN KEY (from_comment_id)
        REFERENCES public.comments (id),
    CONSTRAINT mentioned_user_id_fkey FOREIGN KEY (mentioned_user_id)
        REFERENCES public.users (id)
);

CREATE INDEX mentions_timestamp_index
    ON public.mentions USING btree ("timestamp");
