use termchat_wan::state::{State, CursorMovement, ChatMessage, MessageType, ScrollMovement};
use termchat_wan::{terminal_events::{TerminalEventCollector},};
use termchat_wan::renderer::{Renderer};
use termchat_wan::action::{Action, Processing};
use termchat_wan::message::{NetMessage, Chunk};
use termchat_wan::util::{Result, Reportable};
use termchat_wan::commands::{CommandManager};
use termchat_wan::commands::send_file::SendFileCommand;
use termchat_wan::commands::whisper_message::SendWhisperCommand;
use termchat_wan::config::Config;
use termchat_wan::events::Event;
use crossterm::event::{Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use message_io::node::{self, NodeEvent, NodeHandler, NodeListener};
use message_io::network::{NetEvent, Endpoint, Transport};
use std::time::Duration;
use std::io::{ErrorKind};
use clap::{App, Arg};
use std::net::{SocketAddrV4};

pub struct Application<'a> {
    config: &'a Config,
    state: State,
    commands: CommandManager,
    handler: NodeHandler<Event>,
    listener: Option<NodeListener<Event>>,
    _terminal_events: TerminalEventCollector,
}

impl<'a> Application<'a> {


    pub fn new(config: &'a Config) -> Result<Application<'a>> {

        let commands = CommandManager::default().with(SendFileCommand).with(SendWhisperCommand);
        let (handler, listener) = node::split::<Event>();
        let h = handler.clone();
        let _terminal_events = TerminalEventCollector::new(move |term_event| match term_event {
            Ok(event) => h.signals().send(Event::Terminal(event)),
            Err(e) => h.signals().send(Event::Close(Some(e))),
        })?;
        
        Ok(Application {
            config,
            state: State::default(),
            commands,
            // Stored because we need its internal thread running until the Application was dropped,
            handler: handler,
            listener: Some(listener),
            _terminal_events,
        })
    }

    pub fn run(&mut self, out: impl std::io::Write) -> Result<()> {
        let mut renderer = Renderer::new(out)?;
        renderer.render(&self.state, &self.config.theme)?;

        let (server, _) = self.handler.network().connect(Transport::FramedTcp, self.config.server_addr)?;

        self.listener.take().unwrap().for_each(move |event| match event {
            NodeEvent::Network(net_event) => match net_event {
                NetEvent::Connected(_endpoint, _ok) => {
                    let message = NetMessage::Hi(self.config.user_name.clone());
                    let payload = bincode::serialize(&message).unwrap();
                    self.handler.network().send(server, payload.as_ref());

                    self.handler.signals().send_with_timer(Event::Network(NetMessage::BBB(self.config.user_name.clone())), Duration::from_secs(3));
                },
                NetEvent::Accepted(_, _) => unreachable!(),
                NetEvent::Message(_endpoint, data) => {
                    let message: NetMessage = bincode::deserialize(data).unwrap();
                    self.handler.signals().send(Event::Network(message));
                },
                NetEvent::Disconnected(_endpoint) => (),
            },
            NodeEvent::Signal(msg) => {
                
                match msg {
                    Event::Network(net_message) => {
                        self.process_network_message(net_message, &server);
                    },
                    Event::Terminal(term_event) => {
                        self.process_terminal_event(term_event, &server);
                    },
                    Event::Action(action) => {
                        self.process_action(action, self.handler.clone(), server);
                    },
                    Event::Close(_) => {
                        self.handler.stop();
                    }
                }
                renderer.render(&self.state, &self.config.theme).unwrap();
            }
        });
        Ok(())
    }

    fn process_network_message(&mut self, message: NetMessage, server: &Endpoint) {
        match message {
            NetMessage::BBB(user) =>{
                let u = user.clone();
                let msg = NetMessage::BBB(user.into());
                let payload = bincode::serialize(&msg).unwrap();
                self.handler.network().send(*server, payload.as_ref());
                self.handler.signals().send_with_timer(Event::Network(NetMessage::BBB(u.into())), Duration::from_secs(10));
            },
            NetMessage::Hi(user) => {
                if user != self.config.user_name {
                    self.state.add_message(ChatMessage::new(user.into(), MessageType::Connection));
                    self.righ_the_bell();
                }
            },
            NetMessage::Bye(user) => {
                if user != self.config.user_name {
                    self.state.add_message(ChatMessage::new(user.into(), MessageType::Disconnection));
                    self.righ_the_bell();
                }
            },
            NetMessage::UserMessage(user, content) => {
                if user != self.config.user_name {
                    self.state.add_message(ChatMessage::new(user.into(), MessageType::Text(content)));
                    self.righ_the_bell();
                }
            }
            NetMessage::WhisperMessage(from, to, content) => {
                if to == self.config.user_name {
                    self.state.add_message(ChatMessage::new(from.into(), MessageType::PrivateText(content)));
                    self.righ_the_bell();
                }
            }
            NetMessage::UserData(from, _to, file_name, chunk) => {
                use std::io::Write;
                    match chunk {
                        Chunk::Error => {
                            format!("'{}' had an error while sending '{}'", from, file_name)
                                .report_err(&mut self.state);
                        }
                        Chunk::End => {
                            format!(
                                "Successfully received file '{}' from user '{}'!",
                                std::env::temp_dir().join("termchat").join(&from).join(file_name).to_str().unwrap(),
                                 from
                            )
                            .report_info(&mut self.state);
                            self.righ_the_bell();
                        }
                        Chunk::Data(data) => {
                            let try_write = || -> Result<()> {
                                let user_path = std::env::temp_dir().join("termchat").join(&from);
                                match std::fs::create_dir_all(&user_path) {
                                    Ok(_) => (),
                                    Err(ref err) if err.kind() == ErrorKind::AlreadyExists => (),
                                    Err(e) => return Err(e.into()),
                                }

                                let file_path = user_path.join(file_name);
                                std::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(file_path)?
                                    .write_all(&data)?;

                                Ok(())
                            };

                            try_write().report_if_err(&mut self.state);
                        }
                    }
            }
        }
    }

    fn process_terminal_event(&mut self, term_event: TermEvent, server: &Endpoint) {
        match term_event {
            TermEvent::FocusGained => (),
            TermEvent::FocusLost => (),
            TermEvent::Paste(_) => (),
            TermEvent::Mouse(_) => (),
            TermEvent::Resize(_, _) => (),
            TermEvent::Key(KeyEvent { code, modifiers, kind, .. }) => match code {
                KeyCode::Esc => {
                    self.handler.signals().send_with_priority(Event::Close(None));
                }
                KeyCode::Char(character) => {
                    if character == 'c' && modifiers.contains(KeyModifiers::CONTROL) {
                        self.handler.signals().send_with_priority(Event::Close(None));
                    }
                    else {
                        match kind {
                            KeyEventKind::Press => {self.state.input_write(character);},
                            _ => ()
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(input) = self.state.reset_input() {
                        match self.commands.find_command_action(&input, self.config).transpose() {
                            Ok(action) => {
                                let message = ChatMessage::new(
                                    format!("{} (me)", self.config.user_name),
                                    MessageType::Text(input.clone()),
                                );
                                self.state.add_message(message);
                                
                                match action {
                                    Some(action) => self.process_action(action, self.handler.clone(), *server),
                                    None => {
                                        if input.starts_with('?') {
                                            String::from("This command doesn't exists")
                                                .report_err(&mut self.state);
                                        }else{
                                            let msg = NetMessage::UserMessage(self.config.user_name.clone(), input.clone());
                                            let payload = bincode::serialize(&msg).unwrap();
                                            self.handler.network().send(*server, payload.as_ref());
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                error.report_err(&mut self.state);
                            }
                        };
                    }
                }
                KeyCode::Delete => {
                    self.state.input_remove();
                }
                KeyCode::Backspace => {
                    self.state.input_remove_previous();
                }
                KeyCode::Left => {
                    self.state.input_move_cursor(CursorMovement::Left);
                }
                KeyCode::Right => {
                    self.state.input_move_cursor(CursorMovement::Right);
                }
                KeyCode::Home => {
                    self.state.input_move_cursor(CursorMovement::Start);
                }
                KeyCode::End => {
                    self.state.input_move_cursor(CursorMovement::End);
                }
                KeyCode::Up => {
                    self.state.messages_scroll(ScrollMovement::Up);
                }
                KeyCode::Down => {
                    self.state.messages_scroll(ScrollMovement::Down);
                }
                KeyCode::PageUp => {
                    self.state.messages_scroll(ScrollMovement::Start);
                }
                _ => (),
            },
        }
    }

    fn process_action(&mut self, mut action: Box<dyn Action>, handler: NodeHandler<Event>, server: Endpoint) {
        match action.process(&mut self.state, handler, server) {
            Processing::Completed => (),
            Processing::Partial(delay) => {
                self.handler.signals().send_with_timer(Event::Action(action), delay);
            }
        }
    }

    pub fn righ_the_bell(&self) {
        if self.config.terminal_bell {
            print!("\x07");
        }
    }
}


fn main(){
    let matches = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            Arg::with_name("server_addr")
                .long("server_addr")
                .short("s")
                .takes_value(true)
                .validator(|addr| match addr.parse::<SocketAddrV4>() {
                    Ok(_) => Ok(()),
                    Err(_) => Err("The value must have syntax ipv4:port".into()),
                })
                .help("the chatroom server"),
        )
        .arg(
            Arg::with_name("username")
                .long("username")
                .takes_value(true)
                .short("u")
                .help("Name used as user idenfication"),
        )
        .arg(
            Arg::with_name("quiet-mode")
                .long("quiet-mode")
                .short("q")
                .help("Disable the terminal bell sound"),
        )
        .arg(
            Arg::with_name("theme")
                .long("theme")
                .validator(|theme| match theme.to_lowercase().as_str() {
                    "dark" | "light" => Ok(()),
                    _ => Err("Theme accepts only dark and light as value".into()),
                })
                .takes_value(true)
                .help("Choose which theme should termchat use, values are dark and light"),
        )
        .get_matches();

    // The next unwraps are safe because we specified a default value and a validator
    let config = Config::from_matches(matches);

    let result = match Application::new(&config) {
        Ok(mut app) => app.run(std::io::stdout()),
        Err(e) => Err(e),
    };

    if let Err(e) = result {
        // app is now dropped we can print to stderr safely
        eprintln!("termchat exited with error: {}", e);
    }
}