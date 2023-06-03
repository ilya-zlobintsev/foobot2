// @generated automatically by Diesel CLI.

diesel::table! {
    auth (name) {
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 255]
        value -> Nullable<Varchar>,
    }
}

diesel::table! {
    channels (id) {
        id -> Unsigned<Bigint>,
        #[max_length = 255]
        platform -> Varchar,
        #[max_length = 255]
        channel -> Varchar,
    }
}

diesel::table! {
    commands (channel_id, name) {
        #[max_length = 255]
        name -> Varchar,
        action -> Text,
        permissions -> Nullable<Text>,
        channel_id -> Unsigned<Bigint>,
        cooldown -> Nullable<Unsigned<Bigint>>,
        triggers -> Nullable<Text>,
        #[max_length = 127]
        mode -> Varchar,
    }
}

diesel::table! {
    eventsub_triggers (id) {
        #[max_length = 255]
        broadcaster_id -> Varchar,
        #[max_length = 255]
        event_type -> Varchar,
        action -> Mediumtext,
        creation_payload -> Longtext,
        #[max_length = 255]
        id -> Varchar,
        #[max_length = 127]
        execution_mode -> Varchar,
    }
}

diesel::table! {
    filters (channel_id, regex) {
        channel_id -> Unsigned<Bigint>,
        #[max_length = 255]
        regex -> Varchar,
        block_message -> Bool,
        #[max_length = 255]
        replacement -> Nullable<Varchar>,
    }
}

diesel::table! {
    hebi_data (channel_id, name) {
        channel_id -> Unsigned<Bigint>,
        #[max_length = 255]
        name -> Varchar,
        value -> Nullable<Text>,
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
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 255]
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
        #[max_length = 255]
        session_id -> Varchar,
        user_id -> Unsigned<Bigint>,
        username -> Text,
    }
}

diesel::joinable!(commands -> channels (channel_id));
diesel::joinable!(filters -> channels (channel_id));
diesel::joinable!(hebi_data -> channels (channel_id));
diesel::joinable!(prefixes -> channels (channel_id));
diesel::joinable!(user_data -> users (user_id));
diesel::joinable!(web_sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    auth,
    channels,
    commands,
    eventsub_triggers,
    filters,
    hebi_data,
    mirror_connections,
    prefixes,
    user_data,
    users,
    web_sessions,
);
