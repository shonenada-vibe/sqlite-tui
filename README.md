# sqlite-tui

A small SQLite workbench for your terminal, written in Rust. Browse tables and views, inspect schemas and cell values, and run SQL without leaving the keyboard.

```sh
# Try the in-memory demo, with sample customers, products, and orders
cargo run --release

# Open an existing database
cargo run --release -- ./app.db

# Open without allowing changes to the database
cargo run --release -- --read-only ./app.db

# Create a new database
cargo run --release -- --create ./new.db
```

Requires a recent stable Rust toolchain and a terminal at least 60 columns × 18 rows. SQLite is bundled; no separate SQLite installation is needed. The demo is disposable and never writes a database file.

To install the executable:

```sh
cargo install --path . --locked
sqlite-tui ./app.db
```

## Controls

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | Switch between objects, results, and SQL editor |
| `↑` / `↓` or `j` / `k` | Select a table or row |
| `←` / `→` or `h` / `l` | Select a result column; scroll wide results |
| `Enter` | Open the selected table or inspect a full cell |
| `[` / `]` | Previous / next table page |
| `PageUp` / `PageDown` | Move ten rows |
| `g` / `G` | First / last row |
| `/` | Filter tables and views by name |
| `s` | Inspect the selected object's `CREATE` statement |
| `r` | Refresh objects and the current table page |
| `i` / `:` | Focus the SQL editor |
| `F5` / `Ctrl+R` | Execute the editor's SQL |
| `?` / `F1` | Open keyboard help outside the editor |
| `Esc` | Close a popup or leave the editor |
| `q` | Quit outside the editor |
| `Ctrl+C` / `Ctrl+Q` | Quit from anywhere |

In the editor, `Enter` inserts a newline, arrow keys move the cursor, `Home` / `End` (or `Ctrl+A` / `Ctrl+E`) move within a line, and `Ctrl+U` clears the text. `Ctrl+P` / `Ctrl+N` browse successful queries from the current session. Pasting multiline SQL is supported in terminals with bracketed paste.

## SQL behavior

- Execute **one statement at a time**. Multiple statements are rejected before execution. Statements with `RETURNING`, CTEs, and `PRAGMA` queries work.
- Table browsing fetches 100 rows per page. Arbitrary query results display the first 1,000 rows and indicate truncation. Add your own `ORDER BY`, `LIMIT`, and `OFFSET` for deterministic query paging; the table browser uses SQLite's natural row order.
- Writes apply immediately unless you explicitly execute `BEGIN`. Execute `COMMIT` to save or `ROLLBACK` to undo an open transaction. The header shows when a transaction is open; quitting rolls it back. `RETURNING` writes complete even when displayed rows are capped.
- Queries run on the UI thread, with a default 10-second SQLite execution timeout. The interface waits until the query finishes or times out. Change this with `--timeout 30`. Database lock waits are capped at two seconds. This is a SQLite VM execution limit, not a strict wall-clock limit on filesystem operations.
- Tables and views in the `main` database appear in the sidebar. Temporary and attached databases remain accessible through SQL.
- `NULL` values appear as `NULL`; blobs display their byte size. Press `Enter` on a cell to view its full text. Control characters are sanitized for terminal display.
- Missing files are rejected unless `--create` is supplied. `--read-only` opens the database using SQLite's read-only mode.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`db.rs` owns SQLite access and result limits, `app.rs` handles application state and keyboard commands, `editor.rs` implements Unicode-aware text editing, and `ui.rs` renders the interface. Tests cover queries, transactions, pagination, limits, error recovery, keyboard behavior, and rendering at several terminal sizes.

Built with [Ratatui](https://docs.rs/ratatui/0.30.2/ratatui/), Crossterm, and [rusqlite](https://docs.rs/rusqlite/0.40.2/rusqlite/).

## Releases and Homebrew

The [release workflow](.github/workflows/release.yml) follows the release process used by `shonenada-vibe/mdw`. Pushing a stable `vX.Y.Z` tag matching the version in `Cargo.toml`:

1. Tests and builds native binaries for Apple Silicon macOS, Intel macOS, and x86-64 Linux, using the committed `Cargo.lock`.
2. Publishes a GitHub release with a `.tar.gz` archive and SHA-256 checksum for each platform.
3. Opens a PR against `shonenada/homebrew-tap` to add or update `Formula/sqlite-tui.rb`. Merging that PR makes the version available through Homebrew.

Configure a repository secret named `HOMEBREW_TAP_GITHUB_TOKEN` before releasing. Use a token with **Contents: read and write** and **Pull requests: read and write** access to `shonenada/homebrew-tap`, as required by [create-pull-request](https://github.com/peter-evans/create-pull-request#token). Without it, the GitHub release can finish, but the tap update job fails with a setup message. After configuring the secret, rerun the failed job.

For the initial release (after the workflow is on `main`):

```sh
git tag v0.1.0
git push origin v0.1.0
```

For later releases, update `Cargo.toml`, run `cargo check` to refresh `Cargo.lock`, and commit both before tagging. Prerelease tags are rejected. Archives are named `sqlite-tui-vX.Y.Z-TARGET.tar.gz` and contain the executable and README. macOS binaries target macOS 11 or newer; the Linux binary is built on Ubuntu 22.04 and requires glibc 2.35 or newer. Linux ARM builds are not included.

After the first formula PR is merged:

```sh
brew install shonenada/tap/sqlite-tui
```

To regenerate a formula from published checksums, or from downloaded release artifacts:

```sh
scripts/generate-formula.sh v0.1.0
scripts/generate-formula.sh v0.1.0 ./release-artifacts
python3 -m unittest discover -s scripts -p 'test_*.py'
```
