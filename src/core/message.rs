//! The outside world's answers to the use cases' requests.

use crate::core::usecase::timeline::Tally;

/// An answer to a [`Request`](crate::core::usecase::Request), carried back
/// by `dispatch`.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// The timeline took in what the spool held since the last catch-up.
    CaughtUp(Tally),
    /// The timeline was built again from the whole spool.
    Rebuilt(Tally),
    /// A request could not be carried out.
    Failed { what: &'static str, error: String },
}

impl Message {
    /// The message as a person reads it.
    pub fn describe(&self) -> String {
        match self {
            Self::CaughtUp(tally) => format!("caught up: {}", describe_tally(tally)),
            Self::Rebuilt(tally) => format!("rebuilt: {}", describe_tally(tally)),
            Self::Failed { what, error } => format!("could not {what}: {error}"),
        }
    }
}

fn describe_tally(tally: &Tally) -> String {
    let mut text = format!("{} events taken", tally.taken);
    if tally.newer > 0 {
        text.push_str(&format!(
            ", {} from a newer Pulsline set aside",
            tally.newer
        ));
    }
    if tally.malformed > 0 {
        text.push_str(&format!(", {} malformed lines set aside", tally.malformed));
    }
    text
}
