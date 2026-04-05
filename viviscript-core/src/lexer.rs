//! Lexical analyzer for a simple visual-novel scripting language.
//!
//! The lexer recognises keywords (`scene`, `show`, `choice`, …),
//! string/number literals, Lua blocks and a handful of punctuation
//! tokens.  It also tracks line/column information.
//! 

use std::iter::Peekable;
use std::str::Chars;
use unicode_xid::UnicodeXID;

/// Byte range `[start, end)` that denotes where a token appears in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

/// A single token together with its position in the source file.
#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub tok: TokKind,
    pub span: Span,
}

/// All possible token kinds the lexer can emit.
#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    Character,
    Scene, Show, Hide, Play, Stop, 
    Label, Choice, Lua, Jump, Call,

    If, Else, Elif, EnIf,
    Condition(String),

    EnChoice, EnLua, EnLabel,
    
    LuaBlock(String),
    Ident(String),
    Str(String),
    Num(f64),
    Colon,
    At, Equals, Minus, Dollar,
    Newline,
    Comment(String),
    ParamKey(String),
    Reserved(String),
    Flag(String),
    Eof,

    Screen, EnScreen,
    VBox, EnVBox,
    HBox, EnHBox,
    ZBox, EnZBox,
    Frame, EnFrame,
    Button,
    Image,
    Text,
}

#[macro_export] 
macro_rules! define_content_access {
    ($($var:ident($inner:ty)),* $(,)?) => {
        impl TokKind {
            pub fn as_str(&self) -> Option<&str> {
                match self {
                    $(TokKind::$var(s) => Some(s.as_str()),)*
                    _ => None,
                }
            }
            pub fn into_string(self) -> Option<String> {
                match self {
                    $(TokKind::$var(s) => Some(s),)*
                    _ => None,
                }
            }
        }
    };
}
define_content_access!(
    ParamKey(String),
    Reserved(String),
    Flag(String),
    Str(String),
    Ident(String),
    LuaBlock(String),
    Condition(String),
    Comment(String),
);

/// Lexical mode the lexer is currently in.

/// All tokens that can be produced by the lexer.
pub struct Lexer<'a> {
    /// Character iterator with one-character look-ahead.
    chars: Peekable<Chars<'a>>,
    /// Current line number (1-based).
    line: usize,
    /// Current column number (0-based).
    col: usize,
    /// Are we lexing inside a choice block?
    offset: usize,
}

impl<'a> Lexer<'a> {
    /// 用源码字符串构造词法分析器，从第 1 行开始计数。
    pub fn new(src: &'a str) -> Self {
        Lexer {
            chars: src.chars().peekable(),
            line: 1,
            col: 0,
            offset: 0,
        }
    }
    
    /// Advance the cursor by one character, updating line/column bookkeeping.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next();
        if let Some(ch) = c {
            self.offset += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.col = 0;
            } else {
                self.col += 1;
            }
        }
        c
    }
    
    /// Peek at the next character **without** advancing the cursor.
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    /// Peek `n` characters ahead (0 == current peek).
    fn peek_nth(&mut self, n: usize) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.nth(n)
    }
    
    fn tok(&mut self,tok: TokKind, start: usize) -> Tok{
        Tok { tok, span: Span { start, end: self.offset, line: self.line } }
    }
    
    fn tok_one_str (&mut self,tok: TokKind) -> Tok{
        Tok { tok, span: Span { start: self.offset, end: self.offset+1, line: self.line } }
    }

    /// Discard spaces and tabs, but **stop at newline**.
    fn skip_space_no_nl(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' {
                self.bump();
            } else { 
                break;
            }
        }
    }

    /// Convert an escape sequence into the corresponding character.
    fn consume_escape(&mut self) -> char {
        match self.bump() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('r') => '\r',
            Some('"') => '"',
            Some('\'') => '\'',
            Some('\\') => '\\',
            Some(c) => c,
            None => '\\',
        }
    }

    /// Parse a quoted string until `delim` is reached.
    /// Handles `\"`, `\'`, and other back-slash escapes.
    fn string_literal(&mut self, delim: char) -> String {
        let mut out = String::new();
        while let Some(c) = self.bump() {
            match c {
                '\\' => out.push(self.consume_escape()),
                c if c == delim => break,
                _ => out.push(c),
            }
        }
        out
    }

    /// Parse a triple-quoted string `""" … """`.
    fn triple_quote(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.bump() {
            if c == '"' && self.peek() == Some('"') && self.peek_nth(1) == Some('"') {
                for _ in 0..2{self.bump();}
                break;
            }
            if c == '\\' {
                out.push(self.consume_escape());
            } else {
                out.push(c);
            }
        }
        out
    }
    
    /// Parse the remainder of a `:` line as a string.
    fn colon_line(&mut self) -> String {
        let mut out = String::new();
        self.skip_space_no_nl();
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            out.push(self.bump().unwrap());
        }
        out
    }
    
    fn dollar_line(&mut self) -> String {
        let mut out = String::new();
        self.skip_space_no_nl();
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            out.push(self.bump().unwrap());
        }
        out
    }

    /// Convert an identifier-like sequence into a keyword token or `Ident`.
    fn keyword_or_ident(&mut self, first: char) -> TokKind {
        let mut s = String::from(first);
        while let Some(c) = self.peek() {
            if UnicodeXID::is_xid_continue(c) || c == '_' {
                s.push(self.bump().unwrap());
            } else {
                break;
            }
        }
        match s.as_str() {
            "character" => TokKind::Character,
            "scene" => TokKind::Scene,
            "show" => TokKind::Show,
            "hide" => TokKind::Hide,
            "play" => TokKind::Play,
            "stop" => TokKind::Stop,
            "label" => TokKind::Label,
            "choice" => TokKind::Choice,
            "lua" => TokKind::Lua,
            "jump" => TokKind::Jump,
            "call" => TokKind::Call,

            "if" => TokKind::If,
            "else" => TokKind::Else,
            "elif" => TokKind::Elif,
            "enif" => TokKind::EnIf,

            "enco" => TokKind::EnChoice,
            "enlb" => TokKind::EnLabel,
            "enlua" => TokKind::EnLua,

            "screen" => TokKind::Screen,
            "enscreen" => TokKind::EnScreen,
            "vbox" => TokKind::VBox,
            "envbox" => TokKind::EnVBox,
            "hbox" => TokKind::HBox,
            "enhbox" => TokKind::EnHBox,
            "zbox" => TokKind::ZBox,
            "enzbox" => TokKind::EnZBox,
            "frame" => TokKind::Frame,
            "enframe" => TokKind::EnFrame,
            "button" => TokKind::Button,
            "image" => TokKind::Image,
            "text" => TokKind::Text,

            "with" | "at" | "as"=> TokKind::Reserved(s),
            "loop" | "noloop" => TokKind::Flag(s),
            "volume" | "fade_in" | "fade_out" | "image_tag" | "name" | "voice_tag"=> {
                TokKind::ParamKey(s)
            }
            _ => TokKind::Ident(s),
        }
    }

    /// Slurp everything until the terminating `enlua` keyword.
    fn lua_block(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.bump() {
            let mut look = String::new();
            let mut iter = self.chars.clone();
            for _ in 0..5 {
                if let Some(ch) = iter.next() {
                    look.push(ch);
                } else {
                    break;
                }
            }
            if look == "enlua" {
                break
            }
            out.push(c);
        }
        out
    }
    
    /// Parse a number literal or fall back to an identifier.
    fn number_or_ident(&mut self, first: char) -> TokKind {
        let mut s = String::from(first);
        let mut allow_dot = true;
        let mut allow_exp = true;

        while let Some(&c) = self.chars.peek(){
            match c {
                '0'..='9' => s.push(self.bump().unwrap()),
                '.' if allow_dot => {
                    allow_dot = false;
                    s.push(self.bump().unwrap());
                },
                'e' | 'E' if allow_exp => {
                    allow_exp = false;
                    s.push(self.bump().unwrap());
                    if let Some(&sign) = self.chars.peek() {
                        if sign == '+' || sign == '-' {
                            s.push(self.bump().unwrap());
                        }
                    }
                },
                _ => break,
            }
        }
        
        // If we trail with a letter/underscore, treat the whole thing as ident.
        if let Some(nc) = self.chars.peek() {
            if nc.is_alphabetic() || *nc == '_' {
                while let Some(c) = self.chars.peek() {
                    if c.is_alphanumeric() || *c=='_' {
                        s.push(self.bump().unwrap());
                    } else { break }
                }
                return TokKind::Ident(s);
            }
        }

        let val = s.parse().unwrap_or(0.0);
        TokKind::Num(val)
    }

    fn read_condition_line(&mut self) -> String {
        let mut out = String::new();
        self.skip_space_no_nl();

        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }

            if c == '-' && self.peek_nth(1) == Some('-') {
                break;
            }
            out.push(self.bump().unwrap());
        }

        let trimmed = out.trim();
        if let Some(stripped) = trimmed.strip_suffix(':') {
            stripped.trim().to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Run the lexer to completion and return the full token stream.
    pub fn run(&mut self) -> Vec<Tok> {
        let mut tokens = Vec::new();
        let mut last_was_newline = false;

        loop {
            self.skip_space_no_nl();
            match self.peek() {
                Some('\n') => { self.bump(); continue;},
                _ => break,
            }
        }

        while let Some(c) = self.peek() {
            if c == '\n' {
                if !last_was_newline {
                    tokens.push(self.tok_one_str(TokKind::Newline));
                    last_was_newline = true;
                }
                self.bump();
                continue;
            }
            last_was_newline = false;

            self.normal(&mut tokens);
        }
        
        tokens.push(self.tok_one_str(TokKind::Eof));
        tokens
    }

    /// Normal (top-level) lexing rules.
    fn normal(&mut self, tokens: &mut Vec<Tok>) {
        self.skip_space_no_nl();
        let c = match self.peek() {
            Some(c) => c,
            None => return,
        };

        match c {
            '\n' => {},
            '"' => {
                self.bump();
                if self.peek() == Some('"') && self.peek_nth(1) == Some('"') {
                    for _ in 0..2 {self.bump();}
                    let start = self.offset;
                    let content = self.triple_quote();
                    tokens.push(Tok{tok: TokKind::Str(content),span:Span{start,end:self.offset - 3, line: self.line}});
                } else {
                    let start = self.offset;
                    let content = self.string_literal('"');
                    tokens.push(Tok{tok: TokKind::Str(content),span:Span{start,end:self.offset - 1, line: self.line}});
                }
            }
            '\'' => {
                self.bump();
                let start = self.offset;
                let content = self.string_literal('\'');
                tokens.push(Tok{tok: TokKind::Str(content),span:Span{start,end:self.offset - 1, line: self.line}});
            },
            ':' => {
                let last_tok = tokens.last().map(|t| &t.tok);
                let is_start_of_line = tokens.is_empty() || matches!(last_tok, Some(TokKind::Newline));
                let is_after_ident = matches!(last_tok, Some(TokKind::Ident(_)));

                tokens.push(self.tok_one_str(TokKind::Colon));

                if self.peek_nth(1) == Some('"') && self.peek_nth(2) == Some('"') && self.peek_nth(3) == Some('"') {
                    for _ in 0..4 {self.bump();}
                    let start = self.offset;
                    let mut content = String::new();
                    content.push_str(&self.triple_quote());
                    tokens.push(self.tok(TokKind::Str(content), start)); 
                } else if is_start_of_line || is_after_ident {
                    self.bump(); // 吃掉冒号
                    let start = self.offset;
                    let content = self.colon_line();

                    if !content.is_empty() {
                        tokens.push(self.tok(TokKind::Str(content), start));
                    }
                } else {
                    self.bump();
                }
            }
            '-' if self.peek_nth(1) == Some('-') => {
                let mut comments = String::new();
                for _ in 0..2 {self.bump();}
                let start = self.offset;
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    comments.push(self.bump().unwrap());
                }
                tokens.push(self.tok(TokKind::Comment(comments),start));
            },
            '@' => {
                tokens.push(self.tok_one_str(TokKind::At));
                self.bump();
            },
            '=' => {
                tokens.push(self.tok_one_str(TokKind::Equals));
                self.bump();
            },
            '$' => {
                tokens.push(self.tok_one_str(TokKind::Dollar));
                self.bump();
                let start = self.offset;
                let content = self.dollar_line();
                tokens.push(self.tok(TokKind::LuaBlock(content), start));
            },
            '-' => {
                tokens.push(self.tok_one_str(TokKind::Minus));
                self.bump();
            },
            c if c.is_ascii_digit() => {
                let start = self.offset;
                let ch = self.bump().unwrap();
                let content = self.number_or_ident(ch);
                tokens.push(self.tok(content, start));
            },
            c if UnicodeXID::is_xid_continue(c) || c == '_' => {
                let start = self.offset;
                let ch = self.bump().unwrap();
                let tok = self.keyword_or_ident(ch);

                let is_cond_kw = matches!(tok, TokKind::If|TokKind::Elif);

                tokens.push(self.tok(tok.clone(), start));

                if let TokKind::Lua = tok {
                    let content = self.lua_block();
                    tokens.push(self.tok(TokKind::LuaBlock(content),start + 4));
                } else if is_cond_kw {
                    let cond_start = self.offset;
                    let cond_str = self.read_condition_line();

                    if !cond_str.is_empty() {
                        tokens.push(self.tok(TokKind::Condition(cond_str), cond_start))
                    }
                }
            },
            _ => {
                let c = self.bump().unwrap();
                log::warn!("line {}: unexpected character '{}'", self.line, c);
            }
        }
    }
}