CREATE TABLE users (
    id       INTEGER PRIMARY KEY NOT NULL,
    username TEXT                NOT NULL,
    usertype TEXT                NOT NULL,

    UNIQUE(username, usertype)
);

CREATE TABLE subscriptions (
    id             INTEGER PRIMARY KEY NOT NULL,
    user_id        INTEGER             NOT NULL,
    pilot_username TEXT                NOT NULL,

    UNIQUE(user_id, pilot_username),
    FOREIGN KEY(user_id) REFERENCES users(id)
);
