use crate::action::{Action, Processing};
use crate::commands::{Command};
use crate::state::{State};
use crate::message::{NetMessage, Chunk};
use crate::util::{Result, Reportable};
use message_io::node::NodeHandler;
use std::time::Duration;
use message_io::network::Endpoint;
use crate::events::Event;
use std::path::{Path};
use std::io::{Read};

pub struct SendFileCommand;

impl Command for SendFileCommand {
    fn name(&self) -> &'static str {
        "send"
    }

    fn parse_params(&self, user:String, params: Vec<String>) -> Result<Box<dyn Action>> {
        let to = params.get(0).ok_or("No transfer target")?;
        let param = params.get(1).ok_or("No file specified")?;
        let file_path = shellexpand::full(param)?;
        match SendFile::new(user.as_str(), to.as_str(), &file_path) {
            Ok(action) => Ok(Box::new(action)),
            Err(e) => Err(e),
        }
    }
}

pub struct SendFile {
    from: String,
    to: String,
    file: std::fs::File,
    file_name: String,
    file_size: u64,
    progress_id: Option<usize>,
}

impl SendFile {
    const CHUNK_SIZE: usize = 32768;

    pub fn new(from: &str, to: &str, file_path: &str) -> Result<SendFile> {
        const READ_FILENAME_ERROR: &str = "Unable to read file name";
        let from = from.to_string();
        let to = to.to_string();
        let file_path = Path::new(file_path);
        let file_name = file_path
            .file_name()
            .ok_or(READ_FILENAME_ERROR)?
            .to_str()
            .ok_or(READ_FILENAME_ERROR)?
            .to_string();

        let file_size = std::fs::metadata(file_path)?.len();
        let file = std::fs::File::open(file_path)?;
        if file_size == 0 {
            Err("Don't send blank file".to_string().into())
        }else{
            Ok(SendFile {from, to, file, file_name, file_size, progress_id: None })
        }
    }
}

impl Action for SendFile {
    fn process(&mut self, state: &mut State, handler: NodeHandler<Event>, server: Endpoint) -> Processing {
        if self.progress_id.is_none() {
            let id = state.add_progress_message(&self.file_name, self.file_size);
            self.progress_id = Some(id);
        }

        let mut data = [0; Self::CHUNK_SIZE];
        let (bytes_read, chunk, processing) = match self.file.read(&mut data) {
            Ok(0) => (0, Chunk::End, Processing::Completed),
            Ok(bytes_read) => {
                // We add a minor delay to introduce a rate in the sending.
                let processing = Processing::Partial(Duration::from_micros(100));
                (bytes_read, Chunk::Data(data[..bytes_read].to_vec()), processing)
            }
            Err(error) => {
                format!("Error sending file. error: {}", error).report_err(state);
                (0, Chunk::Error, Processing::Completed)
            }
        };

        state.progress_message_update(self.progress_id.unwrap(), bytes_read as u64);

        let message = NetMessage::UserData(self.from.to_owned(), self.to.to_string(), self.file_name.clone(), chunk);
        let payload = bincode::serialize(&message).unwrap();
        handler.network().send(server, payload.as_ref());
        processing
    }
}
