# IRC Server/Client - AI Agent Instructions

## Project Overview

Rust rewrite of an IRC (Internet Relay Chat) server and client. Work-in-progress - the original C version (at `~/Development/Archive/irc`) is feature-complete; this Rust version has better architecture (HashMap-based storage, UUID-based IDs, type-safe concurrency) but is still missing features.

## Architecture

**Cargo Workspace**: Three crates - `server`, `client`, `shared`

### Server (`server/`)

Multi-threaded TCP server. Each connection = dedicated thread.

**State Management**: Thread-safe shared state via `Arc<Mutex<HashMap<...>>>`:

- `UserTable`: `Uuid` → `User` (all connected users)
- `ChannelTable`: `String` → `Arc<Channel>` (all channels)

**Flow**: `main.rs` spawns threads → `server.rs::handle_connection()` reads messages → `handle_message()` processes commands

**Registration Required**: Users must send `NICK` + `USER` before most commands. Only `USER`, `NICK`, `QUIT` allowed pre-registration. After both set, server sends `RPL_WELCOME` (001) and sets `is_registered = true`.

**Broadcasting Pattern**: Four functions in [server/src/server.rs](../server/src/server.rs) (lines 500-573):

- `send_to_user()`: Single user by UUID
- `send_to_channel()`: All users in a channel (filters by `user.channel == Some(channel)`)
- `broadcast_message()`: All users except one (for QUIT notifications)
- `broadcast_to_all()`: Everyone (for NICK changes)

**Channel Constraint**: Each user can only be in ONE channel (stored in `User.channel: Option<Arc<Channel>>`). Channels created on-demand during JOIN.

### Client (`client/`)

Two-thread model:

- **send_thread**: `rustyline` for input → sends raw IRC messages to server
- **recv_thread**: Receives from server → prints to stdout

No command parsing - sends raw IRC protocol strings.

### Protocol (`server/src/message.rs`)

Format: `[:<prefix>] <command> [<params>] [:<trailing>]`

`Message::from()` parser handles:

- Optional prefix: `:nickname!username@hostname`
- Command → `Command` enum
- Space-separated params + optional trailing param (`:` prefix)

Supported: `USER`, `NICK`, `JOIN`, `PART`, `PRIVMSG`, `LIST`, `AWAY`, `QUIT`, `PING`, `PONG`, `KICK`

Reply codes in `ReplyCode` enum (RFC 1459).

## Build & Run

```bash
# Build workspace
cargo build

# Run server (binds to 127.0.0.1:8080)
cargo run --bin server

# Run client
cargo run --bin client -- <username>

# Run tests
cargo test
```

## Code Conventions

- **Formatting**: [rustfmt.toml](../rustfmt.toml) - `max_width = 100`, `unstable_features = true`
- **Error Handling**: `Result<(), Box<dyn std::error::Error>>` pattern in server command handlers
- **Locking**: Always drop locks explicitly when done to avoid deadlocks (see `drop(lock)` usage)
- **Message Broadcasting**: User's message prefix updated BEFORE broadcasting (line 95-101 in server.rs)

## Known Gaps vs C Version

The Rust version is incomplete. Original C implementation at `~/Development/Archive/irc` has all IRC commands working.

**Implemented in Rust**: `USER`, `NICK`, `AWAY`, `PRIVMSG`, `QUIT`, `JOIN` (partial), `PART` (partial)

**Missing/Incomplete**:

- `LIST`: Marked as `todo!()` - should iterate channels and send RPL_LIST for each with user count, then RPL_LISTEND
- `KICK`: Commented out `todo!()` - needs channel operator logic, removes target user from channel
- `JOIN`: Missing broadcast to channel members when user joins
- `PART`: Missing broadcast to channel members when user leaves
- Channel broadcasting after JOIN/PART not implemented (see TODOs at lines 340, 360)

**C Implementation Details** (reference `~/Development/Archive/irc/src/server.c`):

- Fixed-size arrays (`MAX_CLIENTS=100`, `MAX_CHANNELS=100`) vs Rust's dynamic HashMaps
- `KICK` requires: channel existence check, user in channel check, target in channel check, then broadcast PART
- `LIST` iterates all channels, counts users per channel, sends RPL_LIST (322) per channel + RPL_LISTEND (323)
- AWAY broadcasts to user's current channel when toggled (Rust only notifies the user)
