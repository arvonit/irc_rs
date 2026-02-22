#![allow(non_camel_case_types)]

use std::{
    fmt::{Display, Formatter},
    io::{Error, ErrorKind},
};

#[derive(Debug)]
pub struct Message {
    pub prefix: Option<String>,
    pub command: Command,
    pub params: Vec<String>,
}

#[derive(Debug)]
pub struct Response {
    pub prefix: String,
    pub code: ReplyCode,
    pub params: Vec<String>,
}

#[derive(Debug)]
pub enum Command {
    User,
    Nick,
    Join,
    Kick,
    Part,
    PrivMsg,
    List,
    Away,
    Quit,
    Error,
    Ping,
    Pong,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum ReplyCode {
    RPL_WELCOME = 001,
    RPL_YOURHOST = 002,
    RPL_CREATED = 003,
    RPL_MYINFO = 004,
    RPL_AWAY = 301,
    RPL_UNAWAY = 305,
    RPL_NOWAWAY = 306,
    RPL_WHOISUSER = 311,
    RPL_WHOISSERVER = 312,
    RPL_WHOISOPERATOR = 313,
    RPL_WHOISIDLE = 317,
    RPL_ENDOFWHOIS = 318,
    RPL_WHOISCHANNELS = 319,
    RPL_WHOREPLY = 352,
    RPL_ENDOFWHO = 315,
    RPL_LIST = 322,
    RPL_LISTEND = 323,
    RPL_CHANNELMODEIS = 324,
    RPL_NOTOPIC = 331,
    RPL_TOPIC = 332,
    RPL_NAMREPLY = 353,
    RPL_ENDOFNAMES = 366,
    RPL_MOTDSTART = 375,
    RPL_MOTD = 372,
    RPL_ENDOFMOTD = 376,
    RPL_YOUREOPER = 381,

    ERR_NOSUCHNICK = 401,
    ERR_NOSUCHSERVER = 402,
    ERR_NOSUCHCHANNEL = 403,
    ERR_CANNOTSENDTOCHAN = 404,
    ERR_NORECIPIENT = 411,
    ERR_NOTEXTTOSEND = 412,
    ERR_UNKNOWNCOMMAND = 421,
    ERR_NOMOTD = 422,
    ERR_NONICKNAMEGIVEN = 431,
    ERR_NICKNAMEINUSE = 433,
    ERR_USERNOTINCHANNEL = 441,
    ERR_NOTONCHANNEL = 442,
    ERR_NOTREGISTERED = 451,
    ERR_NEEDMOREPARAMS = 461,
    ERR_ALREADYREGISTRED = 462,
    ERR_PASSWDMISMATCH = 464,
    ERR_UNKNOWNMODE = 472,
    ERR_NOPRIVILEGES = 481,
    ERR_CHANOPRIVSNEEDED = 482,
    ERR_UMODEUNKNOWNFLAG = 501,
    ERR_USERSDONTMATCH = 502,
}

pub trait ToIrc: ToString {
    fn to_irc(&self) -> String {
        format!("{}\r\n", self.to_string())
    }
}

// TODO: Add colon for last param that has spaces in it (I think) when formatting String output
impl Message {
    /// Parse an IRC message from a raw input string. Return a message if the input is formatted
    /// properly. Otherwise, return an error describing the issue.
    pub fn from(raw: &str) -> Result<Self, Error> {
        // Trim line ending from input string
        let mut raw = raw.trim_end();

        // There is a prefix
        let prefix = if raw.starts_with(":") {
            // Remove colon from the beginning of the string
            let (_, text) = raw.split_once(":").unwrap();
            // Cut first part of input out and store it in `prefix`
            let (prefix, text) = Message::get_next_word(text);
            // Set raw to input without prefix
            raw = text;
            // Return prefix
            Some(prefix.to_string())
        } else {
            None
        };

        // Cut command word from string
        let (command, text) = Message::get_next_word(raw);
        if command == "" {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Input string does not contain a command.",
            ));
        }
        // Convert command word to Command enum
        // If the command isn't valid, it'll be parsed as Command::Unknown. This is so that the
        // server can handle sending the response.
        let command = Command::from_str(command);
        // Set raw to input without command
        raw = text;

        let mut params = vec![];
        // Add remaining parts of string to parameters
        while !raw.is_empty() {
            if raw.starts_with(":") {
                let (_, param) = raw.split_once(":").unwrap();
                params.push(param.to_string());
                break;
            } else {
                let (param, text) = Message::get_next_word(raw);
                params.push(param.to_string());
                raw = text;
                // println!("{raw:?}");
            }
        }

        Ok(Message {
            prefix,
            command,
            params,
        })
    }

    pub fn new(prefix: Option<String>, command: Command, params: &[&str]) -> Self {
        Message {
            prefix,
            command,
            params: params.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Return the first subsequence of the string separated by a space as well as the rest of the
    /// string. If the string has no spaces, return the input.
    ///
    /// This is an adjustment to `str::split_once` that returns the original input as well as an
    /// empty string instead of `None`.
    fn get_next_word(input: &str) -> (&str, &str) {
        match input.split_once(" ") {
            Some(pair) => pair,
            None => (input, ""), // String is done
        }
    }
}

impl Response {
    pub fn new(prefix: &str, code: ReplyCode, params: &[&str]) -> Self {
        Response {
            prefix: prefix.to_string(),
            code,
            params: params.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Command {
    pub fn from_str(input: &str) -> Self {
        match input.to_uppercase().as_str() {
            "USER" => Command::User,
            "NICK" => Command::Nick,
            "JOIN" => Command::Join,
            "KICK" => Command::Kick,
            "PART" => Command::Part,
            "PRIVMSG" => Command::PrivMsg,
            "LIST" => Command::List,
            "AWAY" => Command::Away,
            "QUIT" => Command::Quit,
            "PING" => Command::Ping,
            "PONG" => Command::Pong,
            "ERROR" => Command::Error,
            _ => Command::Unknown,
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Flatten list of arguments into a string with a colon for message
        let arguments = self
            .params
            .iter()
            .map(|x| {
                if x.contains(" ") {
                    format!(":{}", x)
                } else {
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if let Some(prefix) = &self.prefix {
            write!(
                f,
                ":{} {} {}",
                prefix,
                self.command.to_string().to_uppercase(),
                arguments
            )
        } else {
            write!(
                f,
                "{} {}",
                self.command.to_string().to_uppercase(),
                arguments
            )
        }
    }
}

impl ToIrc for Message {}

impl Display for Command {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Flatten list of arguments into a string with a colon for message
        let arguments = self
            .params
            .iter()
            .map(|x| {
                if x.contains(" ") {
                    format!(":{}", x)
                } else {
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        write!(f, ":{} {:03} {}", self.prefix, self.code as u16, arguments)
    }
}

impl ToIrc for Response {}
