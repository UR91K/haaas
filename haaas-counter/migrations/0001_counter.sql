CREATE TABLE counter (
  id    INTEGER PRIMARY KEY CHECK (id = 1),
  value INTEGER NOT NULL
);
INSERT INTO counter (id, value) VALUES (1, 0);