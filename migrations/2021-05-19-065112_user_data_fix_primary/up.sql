ALTER TABLE user_data DROP PRIMARY KEY;
ALTER TABLE user_data ADD PRIMARY KEY (user_id, name);