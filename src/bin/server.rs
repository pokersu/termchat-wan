use message_io::node::{self, NodeHandler};
use message_io::network::{Endpoint, NetEvent, Transport};
use std::collections::HashMap;
use termchat_wan::message::NetMessage;
use clap::{App, Arg};


struct OnlineDb{
    user_endpoint_dict: HashMap<String, Endpoint>,
    endpoint_user_dict: HashMap<Endpoint, String>,
}


impl OnlineDb {
    fn new() -> OnlineDb {

        OnlineDb{
            user_endpoint_dict:HashMap::new(),
            endpoint_user_dict:HashMap::new(),
        }
    }

    fn logon(&mut self, user:String, endpoint: Endpoint){
        self.user_endpoint_dict.insert(user.clone(), endpoint);
        self.endpoint_user_dict.insert(endpoint, user);
    }

    fn logoff(&mut self, endpoint:Endpoint) -> Option<String>{
        if self.endpoint_user_dict.contains_key(&endpoint) {
            match self.endpoint_user_dict.get(&endpoint){
                Some(user)=>{self.user_endpoint_dict.remove(user);},
                _ => (),
            }
            self.endpoint_user_dict.remove(&endpoint)
        }else{
            None
        }
    }

    fn all_endpoints(&self) -> Vec<Endpoint>{
        self.user_endpoint_dict.values().into_iter().map(|e|e.to_owned()).collect::<Vec<Endpoint>>()
    }

    fn get_endpoint_by_user(&self, user: &String) -> Option<&Endpoint>{
        self.user_endpoint_dict.get(user)
    }
}

fn main() {

    let matches = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            Arg::with_name("port")
                .long("port")
                .short("t")
                .takes_value(true)
                .validator(|port| match port.parse::<u16>() {
                    Ok(_) => Ok(()),
                    Err(_) => Err("The value must be in range 0..65535".into()),
                })
                .help("Tcp server port "),
        )
        .get_matches();

    let mut port = 5877;
    if let Some(p) = matches.value_of("port"){
        port = p.parse().unwrap();

    }
    let mut db = OnlineDb::new();
    let (handler, listener) = node::split::<()>();
    handler.network().listen(Transport::FramedTcp, String::from("0.0.0.0") + ":" + port.to_string().as_str()).unwrap();
    println!("The server is running on {} ...", port);
    listener.for_each(move |event| match event.network(){
        NetEvent::Connected(_, _) => unreachable!(),
        NetEvent::Accepted(_endpoint, _listener) => {},
        NetEvent::Message(endpoint, data)=>{
            let msg = bincode::deserialize(data).unwrap();
            handle_message(&msg, &handler, &endpoint, &mut db);
        },
        NetEvent::Disconnected(endpoint) => {
            let user = db.logoff(endpoint);
            match user {
                Some(u) => {
                    println!("[LogOff] {}", u);
                    for e in db.all_endpoints().iter(){
                        let message = NetMessage::Bye(u.to_string());
                        let payload = bincode::serialize(&message).unwrap();
                        handler.network().send(e.to_owned(), payload.as_ref());
                    }
                },
                None =>{}
            }
        },
    });
}


fn handle_message(msg: &NetMessage, handler: &NodeHandler<()>, endpoint: &Endpoint, db: &mut OnlineDb){

    match msg {
        NetMessage::BBB(user)=>{
            println!("[BBB] {}", user);
        },
        NetMessage::Hi(user)=>{
            println!("[LogOn] {}", user);
            db.logon(user.clone(), *endpoint);
            for e in db.all_endpoints().iter(){
                let payload = bincode::serialize(msg).unwrap();
                handler.network().send(e.to_owned(), payload.as_ref());
            }
        },
        NetMessage::Bye(user)=>{
            println!("[LogOff] {}", user);
            db.logoff( *endpoint);
            for e in db.all_endpoints().iter(){
                let payload = bincode::serialize(msg).unwrap();
                handler.network().send(e.to_owned(), payload.as_ref());
            }
        },
        NetMessage::UserMessage(user, content)=>{
            println!("[Message] {}, {}", user, content);
            for e in db.all_endpoints().iter(){
                let payload = bincode::serialize(msg).unwrap();
                handler.network().send(e.to_owned(), payload.as_ref());
            }
        },
        NetMessage::WhisperMessage(from, to, content)=>{
            println!("[WhisperMessage] {}, {}, {}", from, to, content);
            match db.get_endpoint_by_user(to) {
                Some(e) => {
                    let payload = bincode::serialize(msg).unwrap();
                    handler.network().send(e.to_owned(), payload.as_ref());
                },
                None => ()
            }
        },
        NetMessage::UserData(from, to, file_name, _)=>{
            println!("[SendFile] {}, {}, {}", from, to, file_name);
            match db.get_endpoint_by_user(to) {
                Some(e) => {
                    let payload = bincode::serialize(msg).unwrap();
                    handler.network().send(e.to_owned(), payload.as_ref());
                },
                None => ()
            }
        },
    }
    
}
