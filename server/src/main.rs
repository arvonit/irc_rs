mod message;
mod server;
mod user;

use dashmap::DashMap;
use std::{net::TcpListener, sync::Arc, thread};
use user::{Channel, User};
use uuid::Uuid;

fn main() {
    let port = 6667; // Default for IRC
    let hostname = format!("127.0.0.1:{port}"); // TODO: Allow for custom port
    let listener = TcpListener::bind(&hostname).expect(&format!("Couldn't bind to {}.", &hostname));
    println!("Listening on {}.", &hostname);

    let users = Arc::new(DashMap::<Uuid, User>::new());
    let channels = Arc::new(DashMap::<String, Arc<Channel>>::new());

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to accept connection: {e}");
                continue;
            }
        };
        let users = users.clone();
        let channels = channels.clone();

        thread::spawn(move || server::handle_connection(stream, users, channels, "127.0.0.1"));
    }
}
