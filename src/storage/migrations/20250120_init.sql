CREATE TABLE IF NOT EXISTS tx (
  id TEXT PRIMARY KEY,
  raw BLOB NOT NULL,
  status TEXT NOT NULL,
  priority INTEGER NOT NULL,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL
);


CREATE TABLE IF NOT EXISTS tx_dependence (
    dependent_id TEXT NOT NULL,
    required_id TEXT NOT NULL,
    PRIMARY KEY (dependent_id, required_id),
    FOREIGN KEY (dependent_id) REFERENCES tx(id),
    FOREIGN KEY (required_id) REFERENCES tx(id)
);

