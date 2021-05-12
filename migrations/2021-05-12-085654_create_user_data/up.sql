CREATE TABLE user_data (
    name VARCHAR(255),
    value VARCHAR(255),
    public BOOLEAN NOT NULL DEFAULT false,
    user_id BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (name, value),
    FOREIGN KEY (user_id) REFERENCES users(id) 
)