#![allow(unused)]
mod message;

use message::Message;
use rustyline::Editor;
use std::{
    env,
    io::{self, Error, ErrorKind, Read, Write},
    net::TcpStream,
    process, str, thread,
};

// fn main() {
//     // let m = Message::from(":arvind!arvind@localhost JOIN #foo").unwrap();
//     // println!("{m:?}");
//     // let vec = vec!["arvind", "private", "yo I'm so cool!"];
//     // println!(
//     //     "{:?}",
//     //     vec.iter()
//     //         .map(|x| {
//     //             if x.contains(" ") {
//     //                 format!(":{}", x)
//     //             } else {
//     //                 x.to_string()
//     //             }
//     //         })
//     //         .collect::<Vec<_>>()
//     //         .join(" ")
//     // );

//     // let error = Error::new(ErrorKind::InvalidData, "bruh what");
//     // println!("{error}")V

//     let text = "";
//     let message = Message::from(text);
//     println!("{message:?}");
//     // println!("{message}");
// }

#[quit::main]
fn main() {
    env_logger::init();

    // Get username from command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: client <username>");
        quit::with_code(1);
    }
    let hostname = "127.0.0.1:6667";
    let username = &args[1];

    // Connect to the server
    let mut reader = TcpStream::connect(hostname).unwrap_or_else(|_| {
        println!("Failed to connect to the server.");
        quit::with_code(1);
    });
    let mut writer = reader.try_clone().expect("Failed to clone stream.");

    // Create send and receive threads
    let send_thread = thread::spawn(move || send_handler(writer));
    let recv_thread = thread::spawn(move || recv_handler(reader));

    // Wait for both threads to terminate
    send_thread.join();
    recv_thread.join();
}

fn send_handler(mut writer: TcpStream) {
    let mut editor = Editor::<()>::new();

    loop {
        // let mut message = match editor.readline("> ") {
        //     Ok(line) => {
        //         editor.add_history_entry(line.as_str());
        //         line
        //     }
        //     Err(err) => {
        //         panic!("{err:?}");
        //     }
        // };
        // Read from stdin
        // let mut message = String::new();
        // print!("> ");
        // io::stdout().flush().expect("Failed to flush stdout.");
        // io::stdin()
        //     .read_line(&mut message)
        //     .expect("Failed to read from stdin.");

        // Read input from stdin using readline
        let mut message = editor.readline("> ").expect("Failed to read from stdin");
        editor.add_history_entry(&message);
        // println!("{message:?}");

        // Build message from input
        // let msg = message_from_input(message.trim_end());

        // Send message to server
        writer
            .write_all(message.as_bytes())
            .expect("Failed to send message to the server.");

        // Exit if user wishes to
        if message.to_lowercase() == "quit" || message.to_lowercase() == "exit" {
            break;
        }
    }
}

fn recv_handler(mut reader: TcpStream) {
    loop {
        // Read response from server
        let mut response = vec![0; shared::MESSAGE_SIZE];
        match reader.read(&mut response) {
            Ok(bytes) => {
                if bytes == 0 {
                    print!("\r");
                    io::stdout().flush().expect("Failed to flush stdout.");
                    break;
                }
            }
            Err(err) => panic!("{err}"),
        };

        // Convert response to a `str` and print it out
        // TODO: Figure out a way to avoid this mess
        let response_str = str::from_utf8(&response)
            .expect("Server sent an invalid UTF-8 message.")
            .replace('\0', "");
        let response_str = response_str.trim_end();

        print!("\r"); // Clear the current line; TODO: this needs some work
        println!("<Server> {:?}", response_str);
        print!("> ");
        io::stdout().flush().expect("Failed to flush stdout.");
    }
}

// fn message_from_input(input: &str) -> Message {
//     // Command
//     if input.starts_with("/") {
//     } else {
//         return Message::from(&format!(":foo PRIVMSG {input}")).unwrap();
//     }

//     Message::from("").unwrap()
// }

struct Prefix {
    username: String,
    realname: String,
    hostname: String,
}

struct User {
    username: String,
}
