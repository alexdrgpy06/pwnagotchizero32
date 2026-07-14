//! Personality system: mood, XP, leveling, faces

use anyhow::Result;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;

/// Personality state
#[derive(Debug, Clone)]
pub struct PersonalityState {
    pub mood: Mood,
    pub xp: u64,
    pub level: u32,
    pub epoch: u64,
    pub handshakes_captured: u64,
    pub blind_epochs: u32,
    pub last_face: String,
    pub face_history: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mood {
    Happy,
    Excited,
    Grateful,
    Motivated,
    Smart,
    Lonely,
    Sad,
    Angry,
    Demotivated,
    Bored,
    Cool,
    Intense,
    Friend,
    Broken,
    Debug,
    Upload,
}

impl Mood {
    pub fn from_score(score: i32) -> Self {
        match score {
            100.. => Mood::Excited,
            70..=99 => Mood::Happy,
            50..=69 => Mood::Motivated,
            30..=49 => Mood::Cool,
            10..=29 => Mood::Bored,
            -9..=9 => Mood::Lonely,
            -29..=-10 => Mood::Sad,
            -49..=-30 => Mood::Demotivated,
            -69..=-50 => Mood::Angry,
            _ => Mood::Broken,
        }
    }
}

/// Personality manager
pub struct Personality {
    config: Arc<Config>,
    state: PersonalityState,
    faces: HashMap<Mood, Vec<String>>,
}

impl Personality {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let mut faces = HashMap::new();

        faces.insert(Mood::Happy, config.personality.happy.clone());
        faces.insert(Mood::Excited, config.personality.excited.clone());
        faces.insert(Mood::Grateful, config.personality.grateful.clone());
        faces.insert(Mood::Motivated, config.personality.motivated.clone());
        faces.insert(Mood::Smart, config.personality.smart.clone());
        faces.insert(Mood::Lonely, config.personality.lonely.clone());
        faces.insert(Mood::Sad, config.personality.sad.clone());
        faces.insert(Mood::Angry, config.personality.angry.clone());
        faces.insert(Mood::Demotivated, config.personality.demotivated.clone());
        faces.insert(Mood::Friend, config.personality.friend.clone());
        faces.insert(Mood::Broken, config.personality.broken.clone());
        faces.insert(Mood::Debug, config.personality.debug.clone());
        faces.insert(Mood::Upload, config.personality.upload.clone());
        faces.insert(Mood::Bored, vec!["(・_・)".to_string()]);
        faces.insert(Mood::Cool, vec!["(⌐■_■)".to_string()]);
        faces.insert(Mood::Intense, vec!["(•̀ᴗ•́)و".to_string()]);

        let state = PersonalityState {
            mood: Mood::Happy,
            xp: 0,
            level: 1,
            epoch: 0,
            handshakes_captured: 0,
            blind_epochs: 0,
            last_face: "happy".to_string(),
            face_history: Vec::new(),
        };

        Ok(Self {
            config: config.clone(),
            state,
            faces,
        })
    }

    pub fn get_face(&self, mood: Mood) -> String {
        if let Some(face_list) = self.faces.get(&mood) {
            if !face_list.is_empty() {
                let idx = rand::thread_rng().gen_range(0..face_list.len());
                return face_list[idx].clone();
            }
        }
        "(•‿‿•)".to_string()
    }

    pub fn current_face(&self) -> &str {
        &self.state.last_face
    }

    pub fn current_mood(&self) -> Mood {
        self.state.mood
    }

    /// A short status phrase for the current mood, for the name/phrase line
    /// next to the face — config.toml only carries face-emoji pools (ported
    /// from pwnagotchi's own schema), never text phrases, so this is a
    /// small fixed table rather than another config section.
    pub fn get_phrase(&self) -> &'static str {
        let pool: &[&str] = match self.state.mood {
            Mood::Excited => &["Hack the Planet!", "Let's go!", "So many APs!"],
            Mood::Happy | Mood::Grateful | Mood::Friend => {
                &["Hack the Planet!", "Feeling good", "Thanks for the WiFi"]
            }
            Mood::Motivated | Mood::Smart => &["Learning...", "Getting smarter"],
            Mood::Cool => &["Just cruising"],
            Mood::Bored | Mood::Lonely => &["Anyone out there?", "So quiet..."],
            Mood::Sad | Mood::Demotivated => &["Where is everyone?"],
            Mood::Angry => &["Ugh."],
            Mood::Broken => &["Something's wrong"],
            Mood::Intense => &["Focused."],
            Mood::Debug => &["Debugging..."],
            Mood::Upload => &["Uploading..."],
        };
        pool[rand::thread_rng().gen_range(0..pool.len())]
    }

    pub fn blind_epochs(&self) -> u32 {
        self.state.blind_epochs
    }

    pub fn update_on_handshake(&mut self) {
        self.state.handshakes_captured += 1;
        self.state.xp += 100;
        self.recalculate_mood();
        self.check_level_up();
    }

    pub fn update_on_blind_epoch(&mut self) {
        self.state.blind_epochs += 1;
        if self.state.blind_epochs > 5 {
            self.state.xp = self.state.xp.saturating_sub(10);
        }
        self.recalculate_mood();
    }

    pub fn update_on_upload(&mut self) {
        self.state.xp += 50;
        self.recalculate_mood();
    }

    fn recalculate_mood(&mut self) {
        let score = (self.state.xp as i32 / 10) - (self.state.blind_epochs as i32 * 5);
        self.state.mood = Mood::from_score(score);
    }

    fn check_level_up(&mut self) {
        let new_level = (self.state.xp / 1000) as u32 + 1;
        if new_level > self.state.level {
            self.state.level = new_level;
        }
    }

    pub fn next_epoch(&mut self) {
        self.state.epoch += 1;
        self.state.blind_epochs = 0;
    }

    pub fn get_stats(&self) -> PersonalityStats {
        PersonalityStats {
            mood: self.state.mood,
            xp: self.state.xp,
            level: self.state.level,
            epoch: self.state.epoch,
            handshakes: self.state.handshakes_captured,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersonalityStats {
    pub mood: Mood,
    pub xp: u64,
    pub level: u32,
    pub epoch: u64,
    pub handshakes: u64,
}
