use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug,Clone)]
pub enum LuaCommand {
    Jump(String),
    SaveGlobal,
    SetVolume { channel: String, value: f32 },
    ModifyVisual {
        target: String,
        props: HashMap<String, f32>,
        duration: f32,
        easing: String,
    },
    RegisterLayout { name: String, config: crate::event::LayoutConfig },
    RegisterTransition { name: String, config: crate::event::TransitionConfig },
    MarkDynamic { name: String },
    ShowScreen { id: String, overlay: bool },
    HideScreen(String),
}

#[derive(Debug,Clone)]
pub struct CommandBuffer {
    queue: Arc<Mutex<Vec<LuaCommand>>>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push(&self, cmd: LuaCommand) {
        if let Ok(mut q) = self.queue.lock() {
            q.push(cmd);
        }
    }

    pub fn drain(&self) -> Vec<LuaCommand> {
        if let Ok(mut q) = self.queue.lock() {
            std::mem::take(&mut q)
        } else {
            vec![]
        }
    }
}