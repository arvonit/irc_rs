# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust rewrite of an IRC (Internet Relay Chat) server and client originally written in C (located at `~/Development/Archive/irc`). The project is structured as a Cargo workspace with three crates:

- **server**: IRC server implementation
- **client**: Terminal-based IRC client
- **shared**: Common utilities and constants shared between server and client

**Note**: This is a work-in-progress rewrite. The original C implementation is feature-complete with all core IRC commands working. The Rust version has better architecture (HashMap-based storage vs fixed arrays, UUID-based IDs, type-safe concurrency) but is still missing several features from the C version.

## Building and Running

### Build all workspace members
```bash
cargo build
```

### Run the server
```bash
cargo run --bin server
```
The server binds to `127.0.0.1:8080` by default.

### Run the client
```bash
cargo run --bin client -- <username>
```
The client connects to `127.0.0.1:8080` by default.

### Run tests
```bash
cargo test
```

## Architecture

### Server Architecture

The server uses a multi-threaded architecture:

- **main.rs**: Entry point that binds a TCP listener and spawns a thread per connection
- **server.rs**: Core connection handling logic in `handle_connection()` and `handle_message()`
- **user.rs**: `User` and `Channel` data structures
- **message.rs**: IRC protocol message parsing and serialization

#### State Management

The server maintains shared state using `Arc<Mutex<HashMap<...>>>`:
- `UserTable`: Maps `Uuid` -> `User` for all connected users
- `ChannelTable`: Maps channel name `String` -> `Arc<Channel>` for all channels

Each user has a unique UUID and maintains their own TcpStream for communication.

#### Registration Flow

Users must register before sending most commands:
1. Client sends `NICK <nickname>` to set nickname
2. Client sends `USER <username> ...` to set username
3. Server validates both are set and sends `RPL_WELCOME` (001)
4. User is marked as registered (`is_registered = true`)

Only `USER`, `NICK`, and `QUIT` commands are allowed before registration.

#### Message Broadcasting

The server provides several broadcast functions in server.rs:
- `send_to_user()`: Send to a specific user by UUID
- `send_to_channel()`: Send to all users in a channel
- `broadcast_message()`: Send to all users except one (used for QUIT)
- `broadcast_to_all()`: Send to all connected users (used for NICK changes)

### Client Architecture

The client uses a two-thread model:
- **send_thread**: Reads from stdin using rustyline and sends raw IRC messages to server
- **recv_thread**: Receives messages from server and prints them to stdout

The client currently sends raw IRC protocol messages directly (no command parsing).

### IRC Protocol Implementation

Messages follow IRC protocol format: `[:<prefix>] <command> [<params>] [:<trailing>]`

The `Message::from()` parser in server/src/message.rs handles:
- Optional prefix (`:nickname!username@hostname`)
- Command word (converted to `Command` enum)
- Space-separated parameters
- Optional trailing parameter (prefixed with `:`)

Supported commands: USER, NICK, JOIN, PART, PRIVMSG, LIST, AWAY, QUIT, PING, PONG, KICK

Reply codes are defined in the `ReplyCode` enum matching RFC 1459.

### Channels

Channels are stored in the `ChannelTable` and created on-demand when a user JOINs. Each user can be in at most one channel (stored in `User.channel` field). Channel names start with `#`.

### Message Size

The constant `shared::MESSAGE_SIZE` is set to 1024 bytes and defines the buffer size for all network communication between client and server.

## Implementation Status

### Completed Features

- **USER command**: Registration with username
- **NICK command**: Nickname setting with uniqueness validation
- **PRIVMSG command**: Direct messages and channel messages
- **AWAY command**: Toggle away status with notifications
- **QUIT command**: Graceful disconnection with broadcast
- **Basic JOIN**: Channel creation and user assignment (no broadcast yet)
- **Basic PART**: Remove user from channel (no broadcast yet)

### Missing Features (Present in C Version)

#### High Priority

1. **JOIN/PART broadcasting**: Infrastructure exists (`send_to_channel`, `broadcast_message`) but not connected. When users join/part channels, other channel members should be notified.

2. **LIST command**: Currently marked as `todo!()`. Should list all active channels with user counts. The C version fully implements this.

3. **KICK command**: Enum variant exists but no implementation. Should allow users to kick others from channels.

4. **Welcome message formatting**: The RPL_WELCOME response is sent but may have formatting issues compared to C version.

#### Medium Priority

5. **Client message preprocessing**: The C client auto-formats messages (commands with `/`, regular text as PRIVMSG). Rust client sends raw input.

6. **Client channel context**: C client tracks current channel for convenience. Rust client doesn't maintain this state.

7. **Systematic use of reply codes**: Many IRC reply codes are defined in `ReplyCode` enum but not consistently used throughout the codebase.

#### Lower Priority (Not in C Version Either)

- WHOIS/WHO commands (reply codes defined but not implemented)
- Channel modes and topics
- Operator privileges
- MOTD (Message of the Day)

### Architectural Improvements Over C Version

- **Scalability**: HashMap-based storage instead of fixed-size arrays (C: 100 max clients/channels)
- **Safety**: Rust's type system prevents data races and memory issues
- **ID management**: UUID-based instead of manual integer counters
- **Concurrency**: Arc<Mutex> provides compile-time thread safety guarantees

### Known Issues (Inherited from C Version)

The original C implementation notes networking bugs where messages may be received slightly corrupted, likely due to not consistently using send_all()/recv_all() in loops. The Rust version uses simple `read()` calls and may have similar issues.
