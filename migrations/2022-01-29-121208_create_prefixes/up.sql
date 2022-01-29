DROP TABLE IF EXISTS prefixes;
CREATE TABLE prefixes (
  channel_id BIGINT UNSIGNED PRIMARY KEY,
  prefix TINYTEXT NOT NULL,
  FOREIGN KEY (channel_id) REFERENCES channels(id)
)