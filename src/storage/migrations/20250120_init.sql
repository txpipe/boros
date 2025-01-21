CREATE TABLE IF NOT EXISTS tx (
  id INTEGER AUTO_INCREMENT PRIMARY KEY NOT NULL,
  tx_cbor BLOB NOT NULL,
  status TEXT NOT NULL,
  priority INTEGER NOT NULL,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL
);


CREATE TABLE IF NOT EXISTS tx_dependence (
    dependent_tx_id INTEGER NOT NULL,
    required_tx_id INTEGER NOT NULL,
    PRIMARY KEY (dependent_tx_id, required_tx_id),
    FOREIGN KEY (dependent_tx_id) REFERENCES tx(id),
    FOREIGN KEY (required_tx_id) REFERENCES tx(id)
);

