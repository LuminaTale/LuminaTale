pub struct Typewriter {
    prefix: String,
    full_text: String,
    suffix: String,

    cursor: String,
    blink_timer: f32,

    pub display_text: String,
    chars: Vec<char>,
    progress: f32,
    speed: f32,
    finished: bool,
}

impl Typewriter {
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
            full_text: String::new(),
            suffix: String::new(),

            cursor: String::new(),
            blink_timer: 0.0,

            display_text: String::new(),
            chars: Vec::new(),
            progress: 0.0,
            speed: 30.0,
            finished: true,
        }
    }

    pub fn set_text(&mut self, prefix: &str, text: &str, suffix: &str, cursor: &str) {
        let new_full_match = self.full_text == text;
        if new_full_match { return; }

        self.prefix = prefix.to_string();
        self.full_text = text.to_string();
        self.suffix = suffix.to_string();
        self.cursor = cursor.to_string();

        self.chars = text.chars().collect(); // 拆解为 Unicode 字符
        self.progress = 0.0;
        self.blink_timer = 0.0;
        self.display_text.clear();
        self.finished = text.is_empty();

        self.update_display_text(0);
    }

    pub fn update(&mut self, dt: f32) {
        self.blink_timer += dt;

        if !self.finished {

            self.progress += self.speed * dt;
            let char_count = self.chars.len();

            // 转换 float 进度为 整数索引
            let visible_count = (self.progress as usize).min(char_count);

            self.update_display_text(visible_count);

            if visible_count >= char_count {
                self.finished = true;
            }
        } else {
            let visible_count = self.chars.len();
            self.update_display_text(visible_count);
        }
    }

    fn update_display_text(&mut self, visible_count: usize) {
        let main_part: String = self.chars[0..visible_count].iter().collect();

        let mut final_suffix = self.suffix.clone();

        if self.finished && !self.cursor.is_empty() {
            let blink_speed = 5.0;
            if (self.blink_timer * blink_speed).sin() > 0.0 {
                final_suffix.push_str(&self.cursor);
            }
        }
        self.display_text = format!("{}{}{}", self.prefix, main_part, final_suffix);
    }

    pub fn skip(&mut self) {
        self.progress = self.chars.len() as f32;
        self.display_text = format!("{}{}{}", self.prefix, self.full_text, self.suffix);
        self.finished = true;
    }

    pub fn is_active(&self) -> bool {
        !self.finished
    }
}
