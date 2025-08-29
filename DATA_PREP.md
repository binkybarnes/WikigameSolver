# DATA_PREP.md

# Preparing Wikipedia Data for the Solver

This guide explains how to prepare the preprocessed Wikipedia graph data used by the solver webserver.

## 1. Download Required SQL Dumps

You need the latest English Wikipedia SQL dumps from [https://dumps.wikimedia.org/enwiki/latest/](https://dumps.wikimedia.org/enwiki/latest/):

- `enwiki-latest-linktarget.sql.gz`
- `enwiki-latest-redirect.sql.gz`
- `enwiki-latest-page.sql.gz`
- `enwiki-latest-pagelinks.sql.gz`

Place them in a directory called `sql_files` that is **neighboring the project directory**:

```
project/
  src/
  Cargo.toml
  ...
sql_files/
  enwiki-latest-linktarget.sql.gz
  enwiki-latest-redirect.sql.gz
  enwiki-latest-page.sql.gz
  enwiki-latest-pagelinks.sql.gz
```

Currently the paths are hard-coded in the project:

```rust
build_linktargets_dense("../sql_files/enwiki-latest-linktarget.sql.gz", &title_to_id)?;
```

## 2. Set Up Environment Variables

Create a `.env` file in the project root with the following content (adjust paths if needed):

```
JWT_SECRET=0d9ca0c73b299331b76c6c3bec4f5cadf6937405e25a89b8f5607f5dd478178a
DATABASE_URL=sqlite:/absolute/path/to/app.db
LEADERBOARD_LIMIT=5000
GOOGLE_CLIENT_ID=YOUR_GOOGLE_CLIENT_ID  # OAuth client ID, see https://developers.google.com/identity/sign-in/web/sign-in
API_ANALYTICS_API_KEY=YOUR_API_KEY      # From https://github.com/tom-draper/api-analytics
FRONTEND_ORIGIN=http://localhost:5173
PORT=3000
IS_PRODUCTION=true
```

> The `GOOGLE_CLIENT_ID` and `API_ANALYTICS_API_KEY` are optional in practice. They're included here to avoid runtime errors, but the solver can work without them. I'm just lazy to make them fully optional.

You also need to create the SQLite database file at the path specified by `DATABASE_URL`.

## 3. Rebuild Preprocessed Graph Data

The solver supports rebuilding the data from SQL dumps. To rebuild, run the webserver with the `--rebuild` flag:

```bash
cargo run -- --rebuild
```

## 4. Data Structures

The preprocessed Wikipedia graph data uses **dense IDs** (compact integers for pages). All files are **memory-mapped** for efficient access. The files are:

- **csr/** – Main Wikipedia graph in CSR format. Redirects fully resolved.
- **dense_id_to_title/** – Dense IDs → Wikipedia titles.
- **title_to_dense_id/** – Wikipedia titles → dense IDs.
- **dense_id_to_orig/** – Dense IDs → original page IDs.
- **orig_to_dense_id/** – Original page IDs → dense IDs.
- **redirect_targets_dense/** – Redirect pages (dense IDs) → resolved target pages (dense IDs).
- **redirects_passed/** – Records redirects encountered during traversal. Format: `(page_from, redirect_target) -> redirect`.

You can find the Rust structs for these memory-mapped files in `src/mmap_structs.rs`.

## 5. Using the Webserver

Once the data is prepared, start the webserver normally:

```bash
cargo run --release
```

You can also use the solver's **bi-directional BFS CSR function** without the webserver, but the webserver is included for convenience.

---

This setup ensures your solver has all necessary preprocessed Wikipedia data in memory-mapped form for efficient pathfinding.
