use crate::util::{Error};
use crate::message::{NetMessage};
use crossterm::event::{Event as TermEvent};
use crate::action::{Action};

pub enum Event {
    Network(NetMessage),
    Terminal(TermEvent),
    Action(Box<dyn Action>),
    Close(Option<Error>),
}