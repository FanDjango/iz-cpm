use std::io::{Write, stdout};
use std::time::Duration;

//#[macro_use(defer)] extern crate scopeguard;
use crossterm::terminal;
use crossterm::event;

use super::translate::Adm3aToAnsi;

pub struct Console {
    next_char: Option<u8>,
    translator: Adm3aToAnsi
}

impl Console {
    pub fn new() -> Console {
        terminal::enable_raw_mode().unwrap();
        //defer! {
        //    terminal::disable_raw_mode().unwrap();
        //}
    
        Console {
            next_char: None,
            translator: Adm3aToAnsi::new(),
        }
    }
}

impl Console {
    pub fn status(&mut self) -> bool {
        match self.next_char {
            Some(_) => true,
            None => {
                loop {
                    if event::poll(Duration::from_nanos(100)).unwrap() {
                        let event = event::read().unwrap();
                        let some_ch = event_to_chat(event);
                        if let Some(ch) = some_ch {
                            self.next_char = Some(ch);
                            break true
                        }
                        // The event is not a valid char, ignore and retry
                    } else {
                        break false
                    }
                }
            }
        }
    }

    pub fn read(&mut self) -> u8 {
        match self.next_char {
            Some(ch) => {
                self.next_char = None;
                ch
            },
            None => {
                loop {
                    let event = event::read().unwrap();
                    let some_ch = event_to_chat(event);
                    if let Some(ch) = some_ch {
                        break ch;
                    }
                    // The event is not a valid char, ignore and retry
                }
            }
        }
    }
    
    pub fn put(&mut self, ch: u8) {
        if let Some(sequence) = self.translator.translate(ch) {
            print!("{}", sequence);
            stdout().flush().unwrap();
        }
    }
}

fn event_to_chat(event: event::Event) -> Option<u8> {
    //println!("Event::{:?}\r", event);

    let a = match event {
        event::Event::Key(k) => match k.code {
            event::KeyCode::Char(c) => {
                if k.modifiers == event::KeyModifiers::NONE ||
                        k.modifiers == event::KeyModifiers::SHIFT {
                    if ' ' <= c && c <= '~' {
                        // Valid ASCII, not control, char
                        Some(c as u8)
                    } else {
                        None
                    }
                } else if k.modifiers == event::KeyModifiers::CONTROL {
                    if '`' <= c && c <= '~' {
                        // Valid control range
                        Some(c as u8 - '`' as u8)
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
            event::KeyCode::Backspace => Some(127),
            event::KeyCode::Enter => Some(13),
            event::KeyCode::Left =>  Some(8),
            event::KeyCode::Right => Some(12),
            event::KeyCode::Up => Some(11),
            event::KeyCode::Down => Some(10),
            event::KeyCode::Home => Some(30),
            event::KeyCode::Tab => Some(9),
            event::KeyCode::Esc => Some(27),
            _ => None, // We ignore: End, PageUp, PageDown, BackTab, Delete, Insert, F(n)
        },
        _ => None, // Not a keyboard event, ignore.
    };

    //println!("Ascci: {:?}", a);
    a
}