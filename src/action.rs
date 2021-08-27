use crate::state::{State};
use message_io::node::NodeHandler;
use std::time::Duration;
use message_io::network::Endpoint;
use crate::events::Event;

pub enum Processing {
    Completed,
    Partial(Duration),
}

pub trait Action: Send {
    fn process(&mut self, state: &mut State, handler: NodeHandler<Event>, server: Endpoint) -> Processing;
}
