ALTER TABLE user_data DROP FOREIGN KEY user_data_ibfk_1;
ALTER TABLE user_data ADD FOREIGN KEY (user_id) REFERENCES users(id);