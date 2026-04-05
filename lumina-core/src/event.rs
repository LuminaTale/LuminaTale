use std::collections::HashMap;
use std::sync::Arc;
use viviscript_core::ast::UiStmt;

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub x: f32,       // 0.0~1.0 相对屏幕宽
    pub y: f32,       // 0.0~1.0 相对屏幕高
    pub anchor_x: f32,// 0.0~1.0
    pub anchor_y: f32,// 0.0~1.0
}

#[derive(Debug, Clone)]
pub struct TransitionConfig {
    pub duration: f32,
    pub easing: String,
    pub props: HashMap<String, (Option<f32>, f32)>,
    pub mask_img: Option<String>,
    pub vague: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum OutputEvent {
    ShowNarration { lines: Vec<String> },
    ShowDialogue { name: String, content: String },
    ShowChoice { title: Option<String>, options: Vec<String> },

    PlayAudio {channel: String, path: String, fade_in: f32, volume: f32 ,looping: bool},
    StopAudio {channel: String, fade_out: f32},
    
    NewScene {transition: String},
    NewSprite {
        target: String,
        texture: String,
        pos_str: Option<String>,
        transition: Option<String>,
        attrs: Vec<String>,
        defer_visual: bool,
    },
    UpdateSprite { target:String, transition: String },
    HideSprite { target:String, transition: Option<String> },

    Preload {
        images: Vec<String>,
        audios: Vec<String>,
    },
    SetVolume {
        channel: String,
        value: f32,
    },
    ModifyVisual {
        target: String,
        props: HashMap<String, f32>,
        duration: f32,
        easing: String
    },
    RegisterLayout { name: String, config: LayoutConfig },
    RegisterTransition { name: String, config: TransitionConfig },

    /// vivi screen 定义被执行时，注册到渲染器的 registry
    RegisterScreen { id: String, def: Arc<[UiStmt]> },
    /// 触发显示一个已注册的 screen
    ShowScreen { id: String, overlay: bool },
    /// 隐藏一个 screen
    HideScreen { id: String },

    StepDone,
    End,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    ChoiceMade { index: usize },
    Continue,
    /// 从 UI 按钮触发的 label 跳转
    Jump(String),
    Exit,
    SaveRequest { slot: u32 },
    LoadRequest { slot: u32 },
}