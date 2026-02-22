use crate::{
    message::{Command, Message, ReplyCode, Response, ToIrc},
    user::{Channel, User},
};
use dashmap::DashMap;
use std::{
    io::{Read, Write},
    net::TcpStream,
    str::{self},
    sync::Arc,
};
use uuid::Uuid;

type UserTable = DashMap<Uuid, User>;
type ChannelTable = DashMap<String, Arc<Channel>>;

#[derive(PartialEq)]
enum CommandResponse {
    Continue,
    Quit,
}

pub fn handle_connection(
    mut stream: TcpStream,
    users: Arc<UserTable>,
    channels: Arc<ChannelTable>,
    hostname: &str,
) {
    let address = stream
        .local_addr()
        .expect("Failed to get IP address of client socket.")
        .ip();

    let user = User::new(address, stream.try_clone().unwrap());
    let user_id = user.id; // Created because value is moved into users table
    users.insert(user_id, user);
    println!(
        "New connection from {}. {} active connections.",
        address,
        users.len()
    );

    loop {
        // Wait for message from client
        // TODO: Consider creating a buffered reader and using reader.lines() to process the string
        // that ends with CLRF
        let mut message_ascii = vec![0; shared::MESSAGE_SIZE];
        stream
            .read(&mut message_ascii)
            .expect("Failed to read message from client.");

        // Convert `message` to a String and print it out
        let message_str = str::from_utf8(&message_ascii)
            .expect("Client sent an invalid UTF-8 message.")
            .replace('\0', "");
        println!("Raw Message: {:?}", message_str);

        // Extract IRC command from client input
        let message = match Message::from(&message_str) {
            Ok(message) => {
                println!("Parsed Message: {:?}", message);
                message
            }
            Err(err) => {
                // TODO: Fix reply code
                let response =
                    Response::new(hostname, ReplyCode::ERR_UNKNOWNCOMMAND, &[&err.to_string()]);
                send_to_user(&response, &users, user_id).expect("Failed to send message.");
                continue;
            }
        };

        match handle_message(message, &users, &channels, user_id, hostname) {
            Ok(CommandResponse::Quit) => break,
            Ok(CommandResponse::Continue) => {}
            Err(e) => eprintln!("Error handling message: {e}"),
        }
    }

    // Remove user from the table
    users.remove(&user_id);
}

fn handle_message<'a>(
    mut message: Message,
    users: &'a UserTable,
    channels: &'a ChannelTable,
    user_id: Uuid,
    server_prefix: &str,
) -> Result<CommandResponse, Box<dyn std::error::Error + 'a>> {
    // Check if the user is registered
    let is_registered = {
        // Get a reference to the user in the table
        let user = users.get(&user_id).unwrap();

        // Update message's prefix to the user's in case we need to broadcast this message to other
        // users
        message.prefix = user.prefix();

        // Return it
        user.is_registered
    };

    // In order for a user to become registered, the client has to send a NICK message with a valid
    // nickname and a USER message with their username. If all checks pass, they will receieve a
    // welcome message.

    // Only allow USER, NICK, and QUIT commands if user is not registered
    if !is_registered
        && !matches!(
            message.command,
            Command::User | Command::Nick | Command::Quit
        )
    {
        let response = Response::new(
            server_prefix,
            ReplyCode::ERR_NOTREGISTERED,
            &["You have not registered."],
        );
        send_to_user(&response, &users, user_id)?;
        return Ok(CommandResponse::Continue);
    }

    // Perform command associated with message
    match message.command {
        Command::User => {
            // Example: USER guest 0 * :Ronnie Reagan

            // We will only parse the first argument (username) and ignore the rest
            let username = match message.params.get(0) {
                Some(name) => name.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NONICKNAMEGIVEN,
                        &["No nickname was given."],
                    );
                    send_to_user(&response, &users, user_id)?;

                    return Ok(CommandResponse::Continue);
                }
            };

            // Check if user is already registered
            let is_registered = users
                .get(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .is_registered;

            // If the user is already registered, ignore the request and send ERR_ALREADYREGISTERED
            if is_registered {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_ALREADYREGISTRED,
                    &["Cannot send USER message since the client is already registered."],
                );

                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            // Set username (no longer holding any references)
            users
                .get_mut(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .username = Some(username);
        }
        Command::Nick => {
            // Example: NICK Wiz

            // Get the first parameter in the message
            let nickname = match message.params.get(0) {
                Some(name) => name.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NONICKNAMEGIVEN,
                        &["No nickname was given."],
                    );

                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            if nickname_in_use(&nickname, &users) {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_NICKNAMEINUSE,
                    &["Nickname is already in use."],
                );

                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            // Update nickname and get registration status
            let is_registered = {
                let mut user = users
                    .get_mut(&user_id)
                    .ok_or("Unable to find user in table with given ID.")?;
                user.nickname = Some(nickname);
                user.is_registered
            }; // RefMut dropped here

            // Only broadcast NICK message if user is registered
            if is_registered {
                broadcast_to_all(&message, &users)?;
            }
        }
        Command::Away => {
            // Toggle away status and prepare response
            let is_away = {
                let mut user = users.get_mut(&user_id).unwrap();
                user.is_away = !user.is_away;
                user.is_away
            }; // RefMut dropped here

            let response = if is_away {
                Response::new(
                    server_prefix,
                    ReplyCode::RPL_NOWAWAY,
                    &["You are now away."],
                )
            } else {
                Response::new(
                    server_prefix,
                    ReplyCode::RPL_UNAWAY,
                    &["You are no longer away."],
                )
            };

            send_to_user(&response, &users, user_id)?;
        }
        Command::PrivMsg => {
            // TODO: Do not allow messaging channels if user has not joined it
            // Example: PRIVMSG user :Hello there!
            //          PRIVMSG #channel :Hello there!
            if message.params.len() != 2 {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_NORECIPIENT,
                    &["No recipient for the message was given."],
                );
                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            let recipient = message.params.get(0).unwrap().clone();

            // It's not a channel
            if !recipient.starts_with("#") {
                if let Some(nickname_id) = get_nickname_id(&recipient, &users) {
                    let is_away = users
                        .get(&nickname_id)
                        .ok_or("Unable to find user in table with given ID")?
                        .is_away;
                    if is_away {
                        let response = Response::new(
                            server_prefix,
                            ReplyCode::RPL_AWAY,
                            &[&recipient, "The recipient is marked as away."],
                        );
                        send_to_user(&response, &users, user_id)?;
                    }

                    send_to_user(&message, &users, nickname_id)?;
                } else {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NOSUCHNICK,
                        &["The given nick was not found."],
                    );
                    send_to_user(&response, &users, user_id)?;
                }
            } else {
                let channel = match channels.get(&recipient) {
                    Some(c) => c,
                    None => {
                        let response = Response::new(
                            server_prefix,
                            ReplyCode::ERR_NOSUCHCHANNEL,
                            &["The given channel was not found."],
                        );
                        send_to_user(&response, &users, user_id)?;
                        return Ok(CommandResponse::Continue);
                    }
                };

                let in_channel = users
                    .get(&user_id)
                    .ok_or("Unable to find user in table with given ID.")?
                    .channel
                    .as_ref()
                    .map_or(false, |c| c.name == recipient);

                if !in_channel {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_CANNOTSENDTOCHAN,
                        &["You are not in that channel."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }

                send_to_channel(&message, &users, channel.value(), user_id)?;
            }
        }
        Command::Quit => {
            let acknowledgement_response = Message::new(
                Some(server_prefix.to_string()),
                Command::Error,
                &["User disconnected."],
            );
            send_to_user(&acknowledgement_response, &users, user_id)?;

            // If the user is registered, tell everyone else that the user has left.
            // TODO: ONLY broadcast to users in the same channel(s) as the user
            let is_registered = users
                .get(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .is_registered;
            if is_registered {
                broadcast_message(&message, &users, user_id)?;
            }

            return Ok(CommandResponse::Quit);
        }
        Command::Unknown => {
            let response = Response::new(
                server_prefix,
                ReplyCode::ERR_UNKNOWNCOMMAND,
                &["Unknown command."],
            );
            send_to_user(&response, &users, user_id)?;
        }
        Command::Join => {
            let channel_name = match message.params.get(0) {
                Some(name) => name.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NEEDMOREPARAMS,
                        &["Specify which channel to join."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            // Get a reference to the channel if it is in the channels table, otherwise create it
            let channel = channels
                .entry(channel_name.clone())
                .or_insert(Arc::new(Channel::new(&channel_name)))
                .clone();

            // Set the user's channel to the channel from the table
            users
                .get_mut(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .channel = Some(channel.clone());

            // Broadcast to all users in the channel
            send_to_channel(&message, &users, &channel, user_id)?;
        }
        Command::Part => {
            let channel_name = match message.params.get(0) {
                Some(name) => name.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NEEDMOREPARAMS,
                        &["Specify which channel to leave."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            // Look up channel and check user is actually in it
            let channel = match channels.get(&channel_name) {
                Some(c) => c.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NOSUCHCHANNEL,
                        &["The given channel was not found."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            let in_channel = users
                .get(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .channel
                .as_ref()
                .map(|c| c.name == channel_name)
                .unwrap_or(false);

            if !in_channel {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_NOTONCHANNEL,
                    &["You are not in that channel."],
                );
                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            // Remove user from channel
            users
                .get_mut(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .channel = None;

            // Broadcast to channel after removing user
            send_to_channel(&message, &users, &channel, user_id)?;
        }
        Command::Kick => {
            // Example: KICK #general bob :Using profanity
            let channel_name = match message.params.get(0) {
                Some(name) => name.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NEEDMOREPARAMS,
                        &["Specify a channel and user to kick."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            let target_user = match message.params.get(1) {
                Some(user) => user.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NEEDMOREPARAMS,
                        &["Specify a user to kick."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            // Verify channel exists
            let channel = match channels.get(&channel_name) {
                Some(c) => c.clone(),
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NOSUCHCHANNEL,
                        &["The given channel was not found."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            // Check if kicker is in the channel
            let kicker_in_channel = users
                .get(&user_id)
                .ok_or("Unable to find user in table with given ID.")?
                .channel
                .as_ref()
                .map_or(false, |c| c.name == channel_name);

            if !kicker_in_channel {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_NOTONCHANNEL,
                    &["You are not in that channel."],
                );
                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            // Find target user ID
            let target_id = match get_nickname_id(&target_user, &users) {
                Some(id) => id,
                None => {
                    let response = Response::new(
                        server_prefix,
                        ReplyCode::ERR_NOSUCHNICK,
                        &["The given user was not found."],
                    );
                    send_to_user(&response, &users, user_id)?;
                    return Ok(CommandResponse::Continue);
                }
            };

            // Check target is in the channel
            let target_in_channel = users
                .get(&target_id)
                .ok_or("Unable to find target user in table with given ID.")?
                .channel
                .as_ref()
                .map_or(false, |c| c.name == channel_name);

            if !target_in_channel {
                let response = Response::new(
                    server_prefix,
                    ReplyCode::ERR_USERNOTINCHANNEL,
                    &["That user is not in the channel."],
                );
                send_to_user(&response, &users, user_id)?;
                return Ok(CommandResponse::Continue);
            }

            // Broadcast KICK to channel
            send_to_channel(&message, &users, &channel, user_id)?;

            // Remove target from channel
            users
                .get_mut(&target_id)
                .ok_or("Unable to find target user in table with given ID.")?
                .channel = None;
        }
        Command::List => {
            // Send one RPL_LIST per channel, then RPL_LISTEND
            for entry in channels.iter() {
                let channel = entry.value();
                let user_count = users
                    .iter()
                    .filter(|user| {
                        user.channel // It really isn't necessary to call value() first as done above
                            .as_ref()
                            .map_or(false, |c| c.name == channel.name)
                    })
                    .count();

                // Send RPL_LIST for this channel
                let response = Response::new(
                    server_prefix,
                    ReplyCode::RPL_LIST,
                    &[&channel.name, &user_count.to_string()],
                );
                send_to_user(&response, &users, user_id)?;
            }

            // At the end, send RPL_LISTEND
            let response = Response::new(server_prefix, ReplyCode::RPL_LISTEND, &["End of LIST"]);
            send_to_user(&response, &users, user_id)?;
        }
        Command::Ping => {
            // Ignore any parameters and send back a PONG message
            let response = Message::new(
                Some(server_prefix.to_string()),
                Command::Pong,
                &[server_prefix],
            );
            send_to_user(&response, &users, user_id)?;
        }
        Command::Pong | Command::Error => {}
        _ => send_to_user(&message, &users, user_id)?,
    }

    // Send welcome message if user has completed registration (has both nick and username)

    let user = users
        .get(&user_id)
        .ok_or("Unable to find user in table with given ID.")?;
    let should_register = !user.is_registered && user.prefix().is_some();
    let prefix = user.prefix();
    drop(user); // Most drop explicitly here

    if should_register {
        let prefix = prefix.unwrap();
        let mut user = users
            .get_mut(&user_id)
            .ok_or("Unable to find user in table with given ID.")?;
        user.is_registered = true;
        let response = Response::new(
            &prefix,
            ReplyCode::RPL_WELCOME,
            &[
                user.nickname.as_ref().unwrap(),
                &format!("Welcome to the Internet Relay Network {}", prefix),
            ],
        );
        user.stream.write_all(response.to_irc().as_bytes())?;
    }

    Ok(CommandResponse::Continue)
}

/// This mutates the user table by writing with the stream
pub fn send_to_user<'a, T: ToIrc>(
    message: &T,
    users: &'a UserTable,
    id: Uuid,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    Ok(users
        .get_mut(&id)
        .ok_or("Invalid ID given. User not found in table.")?
        .stream
        .write_all(message.to_irc().as_bytes())?)
}

/// This mutates the user table by writing with the stream
pub fn send_to_channel<'a, T: ToIrc>(
    message: &T,
    users: &'a UserTable,
    channel: &Arc<Channel>,
    id_to_exclude: Uuid,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    // Ok(users
    //     .iter_mut()
    //     .filter(|(_, user)| user.channel == Some(channel.clone()))
    //     .for_each(|(_, user)| user.stream.write_all(message.to_irc().as_bytes()).unwrap()))

    for mut entry in users.iter_mut() {
        let id = *entry.key();
        let user = entry.value_mut();
        if id != id_to_exclude && user.channel == Some(channel.clone()) {
            user.stream.write_all(message.to_irc().as_bytes())?;
        }
    }

    Ok(())
}

/// This mutates the user table by writing with the stream
pub fn broadcast_message<'a, T: ToIrc>(
    message: &T,
    users: &'a UserTable,
    id_to_exclude: Uuid,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    // Ok(users
    //     .iter_mut()
    //     .filter(|(id, _)| **id != id_to_exclude)
    //     .for_each(|(_, user)| user.stream.write_all(message.to_irc().as_bytes()).unwrap()))

    for mut entry in users.iter_mut() {
        let id = *entry.key();
        let user = entry.value_mut();
        if id != id_to_exclude {
            user.stream.write_all(message.to_irc().as_bytes())?
        }
    }

    Ok(())
}

/// This mutates the user table by writing with the stream
pub fn broadcast_to_all<'a, T: ToIrc>(
    message: &T,
    users: &'a UserTable,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    // Ok(users
    //     .iter_mut()
    //     .for_each(|mut entry| entry.stream.write_all(message.to_irc().as_bytes()).unwrap()))

    for mut entry in users.iter_mut() {
        let user = entry.value_mut();
        user.stream.write_all(message.to_irc().as_bytes())?;
    }

    Ok(())
}

pub fn nickname_in_use(nickname: &str, users: &UserTable) -> bool {
    for entry in users.iter() {
        let user = entry.value();
        if let Some(name) = &user.nickname
            && name == nickname
        {
            return true;
        }
    }

    return false;
}

pub fn get_nickname_id(nickname: &str, users: &UserTable) -> Option<Uuid> {
    for entry in users.iter() {
        let id = entry.key();
        let user = entry.value();
        if let Some(name) = &user.nickname {
            if name == nickname {
                return Some(*id);
            }
        }
    }

    return None;
}
