-- Your SQL goes here
CREATE TABLE hebi_data (
    channel_id BIGINT UNSIGNED,
    name VARCHAR(255),
    value TEXT,
    PRIMARY KEY(channel_id, name),
    FOREIGN KEY (channel_id) REFERENCES channels(id)
);
