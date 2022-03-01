CREATE TABLE mirror_connections(
    from_channel_id BIGINT UNSIGNED,
    to_channel_id BIGINT UNSIGNED,
    PRIMARY KEY(from_channel_id, to_channel_id),
    FOREIGN KEY (from_channel_id) REFERENCES channels(id),
    FOREIGN KEY (to_channel_id) REFERENCES channels(id)
)