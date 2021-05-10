CREATE TABLE commands (
    name VARCHAR(255) NOT NULL,
    action TEXT NOT NULL,
    permissions TEXT,
    channel_id BIGINT UNSIGNED,
    PRIMARY KEY (channel_id, name),
    FOREIGN KEY (channel_id) REFERENCES channels(id)
)