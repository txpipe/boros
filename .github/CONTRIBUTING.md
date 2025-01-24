# Contributing

## Getting started

```sh
git clone git@github.com:txpipe/boros.git
cd boros
```

### Storage

Set up the SQLite. SQLx is used to manage SQL queries. Install SQLx CLI

```
cargo install sqlx-cli
```

Init SQLx

```sh
# Set db path env
export DATABASE_URL="sqlite:dev.db"

# Create database
cargo sqlx db create

# Start migrations
cargo sqlx migrate run --source ./src/storage/migrations
```

If there are updates in the schemas, execute the command below to update the SQLx map files

```sh
cargo sqlx prepare
```

### Run

Command to run Boros 

```sh
cargo run 
```
