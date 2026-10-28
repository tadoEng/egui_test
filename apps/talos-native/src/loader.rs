//! Runs [`Effect::Load`] off the UI thread and returns each response with the
//! ticket it was issued for. The loader never decides whether a response is
//! still wanted; `State::apply` does.
use crate::document::Document;
use crate::state::{Effect, Ticket};
use std::sync::{Arc, mpsc};

pub type Response = (Ticket, Result<Arc<Document>, String>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadMode {
    /// One short-lived thread per request (native builds).
    Background,
    /// Load during the request; responses are applied on the next frame.
    Inline,
    /// Hold requests until a test releases them, in any order.
    Manual,
}

pub struct Loader {
    mode: LoadMode,
    sender: mpsc::Sender<Response>,
    receiver: mpsc::Receiver<Response>,
    held: Vec<Effect>,
    repaint: Option<egui::Context>,
}
impl Loader {
    pub fn new(mode: LoadMode, repaint: Option<egui::Context>) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            mode,
            sender,
            receiver,
            held: Vec::new(),
            repaint,
        }
    }
    pub fn run(&mut self, effect: Effect) {
        let Effect::Load { ticket, entry } = effect.clone();
        match self.mode {
            LoadMode::Manual => self.held.push(effect),
            LoadMode::Inline => self.respond(ticket, &entry),
            LoadMode::Background => {
                let sender = self.sender.clone();
                let repaint = self.repaint.clone();
                let failed = (ticket.clone(), sender.clone());
                let spawned = std::thread::Builder::new()
                    .name("talos-load".to_owned())
                    .spawn(move || {
                        let result = Document::load(&entry).map(Arc::new);
                        // A closed channel means the app is gone; nothing to tell.
                        sender.send((ticket, result)).ok();
                        if let Some(ctx) = repaint {
                            ctx.request_repaint();
                        }
                    });
                if let Err(error) = spawned {
                    let (ticket, sender) = failed;
                    sender
                        .send((ticket, Err(format!("Could not start the loader: {error}"))))
                        .ok();
                }
            }
        }
    }
    fn respond(&self, ticket: Ticket, entry: &crate::project::VersionEntry) {
        let result = Document::load(entry).map(Arc::new);
        self.sender.send((ticket, result)).ok();
        if let Some(ctx) = &self.repaint {
            ctx.request_repaint();
        }
    }
    /// Responses that have arrived, oldest first.
    pub fn drain(&self) -> Vec<Response> {
        self.receiver.try_iter().collect()
    }
    /// Requests held in [`LoadMode::Manual`], oldest first.
    pub fn held(&self) -> &[Effect] {
        &self.held
    }
    /// Completes one held request (by index into [`Self::held`]).
    pub fn release(&mut self, index: usize) {
        if index < self.held.len() {
            let Effect::Load { ticket, entry } = self.held.remove(index);
            self.respond(ticket, &entry);
        }
    }
}
