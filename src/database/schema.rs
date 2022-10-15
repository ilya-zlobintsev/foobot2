// @generated automatically by Diesel CLI.

diesel::table! {
    auth (name) {
        name -> Varchar,
        value -> Nullable<Varchar>,
    }
}

diesel::table! {
    channels (id) {
        id -> Unsigned<Bigint>,
        platform -> Varchar,
        channel -> Varchar,
    }
}

diesel::table! {
    commands (channel_id, name) {
        name -> Varchar,
        action -> Text,
        permissions -> Nullable<Text>,
        channel_id -> Unsigned<Bigint>,
        cooldown -> Nullable<Unsigned<Bigint>>,
        triggers -> Nullable<Text>,
    }
}

diesel::table! {
    eventsub_triggers (id) {
        broadcaster_id -> Varchar,
        event_type -> Varchar,
        action -> Mediumtext,
        creation_payload -> Longtext,
        id -> Varchar,
    }
}

diesel::table! {
    filters (channel_id, regex) {
        channel_id -> Unsigned<Bigint>,
        regex -> Varchar,
        block_message -> Bool,
        replacement -> Nullable<Varchar>,
    }
}

diesel::table! {
    mirror_connections (from_channel_id, to_channel_id) {
        from_channel_id -> Unsigned<Bigint>,
        to_channel_id -> Unsigned<Bigint>,
    }
}

diesel::table! {
    prefixes (channel_id) {
        channel_id -> Unsigned<Bigint>,
        prefix -> Tinytext,
    }
}

diesel::table! {
    user_data (user_id, name) {
        name -> Varchar,
        value -> Varchar,
        public -> Bool,
        user_id -> Unsigned<Bigint>,
    }
}

diesel::table! {
    users (id) {
        id -> Unsigned<Bigint>,
        twitch_id -> Nullable<Text>,
        discord_id -> Nullable<Text>,
        irc_name -> Nullable<Text>,
        local_addr -> Nullable<Text>,
        telegram_id -> Nullable<Text>,
        matrix_id -> Nullable<Text>,
    }
}

diesel::table! {
    web_sessions (session_id) {
        session_id -> Varchar,
        user_id -> Unsigned<Bigint>,
        username -> Text,
    }
}

diesel::joinable!(commands -> channels (channel_id));
diesel::joinable!(filters -> channels (channel_id));
diesel::joinable!(prefixes -> channels (channel_id));
diesel::joinable!(user_data -> users (user_id));
diesel::joinable!(web_sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    auth,
    channels,
    commands,
    eventsub_triggers,
    filters,
    mirror_connections,
    prefixes,
    user_data,
    users,
    web_sessions,
);
