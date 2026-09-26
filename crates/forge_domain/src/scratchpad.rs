//! harness: R-CTX-10 (D-087) — the agent's scratchpad: short notes the model
//! writes with `write_note`, kept on the conversation's metrics rather than in
//! `Context.messages`, so no compaction stage can summarise or drop them. The
//! harness shows them again whenever compaction has removed them from view
//! (`forge_app::hooks::NotesHandler`), and every note is also appended to the
//! event log as a `ThreadEvent::Note`, so an evicted note is never lost.

use serde::{Deserialize, Serialize};

/// Most notes kept at once; the oldest is evicted first.
pub const MAX_NOTES: usize = 20;

/// Longest note kept, in characters; longer notes are cut with a marker.
pub const MAX_NOTE_CHARS: usize = 500;

/// One scratchpad note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScratchNote {
    /// Position among every note this conversation ever wrote, from 1; never
    /// reused, so `[note N]` names one note for the whole run.
    pub id: u64,
    /// The note, redacted by the caller and cut to [`MAX_NOTE_CHARS`].
    pub text: String,
}

impl ScratchNote {
    /// The label a note is shown under, in tool results and reminders.
    pub fn label(&self) -> String {
        format!("[note {}]", self.id)
    }
}

/// The notes currently kept, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notes {
    /// Kept notes, oldest first, at most [`MAX_NOTES`].
    #[serde(default)]
    pub items: Vec<ScratchNote>,
    /// How many notes were ever written; the next note's id is one more.
    #[serde(default)]
    pub written: u64,
}

impl Notes {
    /// Whether no note is kept.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Keeps `text` as a new note, cutting it to [`MAX_NOTE_CHARS`] and
    /// evicting the oldest note beyond [`MAX_NOTES`].
    ///
    /// Returns the note as kept, and the note evicted to make room, if any.
    ///
    /// # Arguments
    /// * `text` - The note, already redacted.
    ///
    /// # Errors
    /// Returns an error if `text` is blank.
    pub fn add(&mut self, text: &str) -> anyhow::Result<(ScratchNote, Option<ScratchNote>)> {
        let text = text.trim();
        if text.is_empty() {
            anyhow::bail!("A note cannot be empty");
        }
        let text = if text.chars().count() > MAX_NOTE_CHARS {
            let cut: String = text.chars().take(MAX_NOTE_CHARS).collect();
            format!("{cut} [cut at {MAX_NOTE_CHARS} characters]")
        } else {
            text.to_string()
        };
        self.written += 1;
        let note = ScratchNote { id: self.written, text };
        self.items.push(note.clone());
        let evicted = (self.items.len() > MAX_NOTES).then(|| self.items.remove(0));
        Ok((note, evicted))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_notes_get_stable_ids_and_the_oldest_is_evicted_past_the_cap() {
        let mut fixture = Notes::default();
        for n in 1..=MAX_NOTES {
            fixture.add(&format!("fact {n}")).unwrap();
        }

        let (note, evicted) = fixture.add("one more").unwrap();

        let actual = (note.id, evicted.map(|e| e.id), fixture.items.len(), fixture.items[0].id);
        let expected = (MAX_NOTES as u64 + 1, Some(1), MAX_NOTES, 2);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_a_long_note_is_cut_and_says_so_and_a_blank_one_is_refused() {
        let mut fixture = Notes::default();

        let (actual, _) = fixture.add(&"x".repeat(MAX_NOTE_CHARS + 50)).unwrap();

        assert!(actual.text.starts_with(&"x".repeat(MAX_NOTE_CHARS)));
        assert!(actual.text.ends_with("[cut at 500 characters]"));
        assert!(fixture.add("   ").is_err());
    }
}
