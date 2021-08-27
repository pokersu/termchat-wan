use message_io::network::Endpoint;
use message_io::node::NodeHandler;

use crate::action::{Action, Processing};
use crate::events::Event;
use crate::commands::{Command};
use crate::state::{State};
use crate::util::{Result};
use crate::message::{NetMessage};

pub struct SendWhisperCommand;

impl Command for SendWhisperCommand {
    fn name(&self) -> &'static str {
        "msg"
    }

    fn parse_params(&self, user:String, params: Vec<String>) -> Result<Box<dyn Action>> {
        let to = params.get(0).ok_or("No transfer target")?;
        params.get(1).ok_or("Content is blank")?;
        let len = params.len();
        let content = &params[1..len].join(" ");
        match SendWhisper::new(user.as_str(), to.as_str(), &content) {
            Ok(action) => Ok(Box::new(action)),
            Err(e) => Err(e),
        }
    }
}



pub struct SendWhisper {
    from: String,
    to: String,
    content: String,
}

impl SendWhisper {

    pub fn new(from: &str, to: &str, content: &str) -> Result<SendWhisper> {
        let from = from.to_string();
        let to = to.to_string();
        let content = content.to_string();

        Ok(SendWhisper {from, to, content })
    }
}

impl Action for SendWhisper {
    fn process(&mut self, _: &mut State, handler: NodeHandler<Event>, server: Endpoint) -> Processing {
        let message = NetMessage::WhisperMessage(self.from.to_owned(), self.to.to_string(), self.content.clone());
        let payload = bincode::serialize(&message).unwrap();
        handler.network().send(server, payload.as_ref());
        Processing::Completed
    }
}
