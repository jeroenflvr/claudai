# <img src="static/img/icon.svg" width="50" height="50" align="absmiddle" alt="" /> claudia 

A self-hosted web UI for the Claude API. Single-user, two-factor auth, conversation history stored in DuckDB.

## What it does

- Chat with Claude (any model) through a clean browser UI
- Conversation history persisted to DuckDB — sessions survive restarts
- Sidebar lists past sessions; click one to browse turns
- Dark/light theme toggle
- Two-step login: password → OTP via email (SMTP)
- Deployable behind a reverse proxy at a sub-path (`BASE_PATH`)

## How it works

```
Browser -- POST /chat --> Axum handler
  {message, history,     (sends as POST with X-HTTP-Method-Override: GET header;
   session_id}            the handler rejects requests without that header)
                         ├─ validates session (tower-sessions)
                         ├─ forwards messages to Anthropic API (reqwest)
                         ├─ writes turn to DuckDB (bundled, no external dep)
                         └─ returns rendered HTML fragment (Askama template)
```

Auth flow:
1. POST `/login` — password checked in-process (no DB)
2. On success, a 6-digit OTP is generated and emailed via STARTTLS SMTP
3. POST `/verify` — OTP checked against the session, 5-minute expiry
4. Session cookie marks `auth_step=authenticated`

Static assets (CSS/JS) are embedded in the binary at compile time via `include_str!`.

## Stack

- **Rust 2024**, Axum 0.7, Askama 0.12 (typed templates)
- **DuckDB** — statically linked (`bundled` feature), no external database
- **reqwest** with `rustls-tls` — no OpenSSL
- **lettre** — SMTP, STARTTLS on port 587
- **tower-sessions** — in-memory session store (sessions reset on restart)
- Client: marked.js, highlight.js, plain JS (no framework)

## Configuration

All config is via environment variables. Copy `.env` and fill in:

| Variable | Required | Default | Description |
|---|---|---|---|
| `ANTHROPIC_API_KEY` | yes | — | Anthropic API key |
| `AUTH_PASSWORD` | yes | — | Login password |
| `SMTP_HOST` | yes | — | SMTP server hostname |
| `SMTP_USER` | yes | — | SMTP username |
| `SMTP_PASS` | yes | — | SMTP password |
| `AUTH_EMAIL` | yes | — | Address that receives OTP codes |
| `CLAUDE_MODEL` | no | `claude-opus-4-5` | Model name |
| `SMTP_PORT` | no | `587` | SMTP port |
| `PORT` | no | `3000` | HTTP listen port |
| `BASE_PATH` | no | `` | Sub-path prefix, e.g. `/claudia` |
| `DB_PATH` | no | `claudia.duckdb` | Path to the DuckDB file |

## Build and run

### With Nix (NixOS / nix develop)

```sh
nix develop          # enters shell with Rust + cmake + gcc
cargo build          # dev build
cargo run            # run with .env loaded by dotenvy
make test            # run unit tests
```

### Docker

```sh
cp .env.example .env   # edit .env with real values
make build             # docker build, tags :version and :latest
make run               # docker compose up -d
make release           # build + push to registry
```

The `docker-compose.yml` expects a `.env` file alongside it and mounts a `claudia-data` volume at `/data` for the DuckDB file.

### From source (Linux, no Nix)

Requires: Rust 1.80+, cmake, g++

```sh
cargo build --release
DB_PATH=/var/lib/claudia/db.duckdb ./target/release/claudia
```

## Reverse proxy

Set `BASE_PATH=/claudia` in `.env`, then proxy `location /claudia` to `http://localhost:3000`.

All links, form actions, and static asset URLs are prefixed automatically.

