use crate::{Command, Game, MoveError};

pub struct Engine {
    game: Game,
    undo: Option<Game>,
}

impl Engine {
    pub fn new(game: Game) -> Self {
        Self { game, undo: None }
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    /// Commits an accepted transition and publishes the resulting state view.
    pub fn execute(&mut self, command: Command) -> Result<&Game, MoveError> {
        let next = self.game.updated(command)?;
        let previous = std::mem::replace(&mut self.game, next);
        self.undo = Some(previous);
        Ok(&self.game)
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.take() {
            self.game = previous;
            true
        } else {
            false
        }
    }
}
