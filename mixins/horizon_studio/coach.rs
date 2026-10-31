// The agent in the editor: what an AI agent connected through the MCP
// server (horizon-studio-mcp, over the AI link) does in the player's editor
// while they watch. It opens any script, reads the buffer as the player has
// it, and types its changes into it with a caret of its own. Its state lives
// in the studio's UI (`Ui::coach`); ui.rs hands it the player's input,
// look.rs draws it, link.rs offers it as the link's editor methods, and the
// studio's frame moves it on.
//
// The player comes first: their keys always land where they type, an agent
// never switches their file or tab while they are using the studio, and
// what they are typing is never saved for them.

use super::hstudio::Studio;
use super::hstudio_ui::Input;
use std::path::PathBuf;

/// How long after the player last changed the code, or used the studio at
/// all, they count as busy with it (the studio's clock, seconds).
pub const HANDS_OFF: f64 = 2.0;

/// What AI agents do in the editor, and what the player last did there.
pub struct Coach {
    /// When the player last changed the code (an edit of theirs, not one
    /// that opened another file).
    pub player_at: f64,
    /// When they last pressed a key or a button, typed or turned the wheel.
    pub input_at: f64,
}

impl Default for Coach {
    fn default() -> Self {
        Coach {
            player_at: f64::NEG_INFINITY,
            input_at: f64::NEG_INFINITY,
        }
    }
}

impl Coach {
    /// Whether the player changed the code in the last moment.
    pub fn player_editing(&self, now: f64) -> bool {
        now - self.player_at < HANDS_OFF
    }
    /// Whether the player used the studio in the last moment.
    pub fn player_busy(&self, now: f64) -> bool {
        now - self.input_at < HANDS_OFF
    }
}

/// The editor's file and revision before an input, for `after_input`.
pub struct Before {
    revision: u64,
    path: Option<PathBuf>,
}

/// Before the player's input reaches the studio: it is noted.
pub fn before_input(st: &mut Studio, input: &Input) -> Before {
    let pressed = matches!(
        input,
        Input::Key { down: true, .. } | Input::Text(_) | Input::Button { down: true, .. } | Input::Wheel(_)
    );
    if pressed {
        st.s.ui.coach.input_at = st.s.now;
    }
    Before {
        revision: st.s.editor.revision,
        path: st.s.editor.path.clone(),
    }
}

/// After it: when it changed the code of the file that was open (not by
/// opening another), the player is editing.
pub fn after_input(st: &mut Studio, before: Before) {
    let ed = &st.s.editor;
    if ed.revision != before.revision && ed.path == before.path {
        st.s.ui.coach.player_at = st.s.now;
    }
}
