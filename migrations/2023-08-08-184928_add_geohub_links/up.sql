-- Your SQL goes here
CREATE TABLE geohub_link (
    user_id BIGINT UNSIGNED,
    channel_id BIGINT UNSIGNED,
    geohub_name VARCHAR(255) NOT NULL,
    PRIMARY KEY(user_id, channel_id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (channel_id) REFERENCES channels(id)
);
