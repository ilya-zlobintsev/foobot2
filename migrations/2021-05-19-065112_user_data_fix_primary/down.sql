ALTER TABLE user_data DELETE PRIMARY KEY;
ALTER TABLE user_data ADD PRIMARY KEY (name, value);