//! The dictionary window of R19. A later task writes it. Now Enter on a
//! dictionary row only writes the word to the log.

use crate::log;

pub fn open(word: &str) {
    log::info(format_args!(
        "dictionary window: not written yet, word {word:?}"
    ));
}
