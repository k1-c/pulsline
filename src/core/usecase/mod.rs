//! What a person or an agent can do, independent of how they asked — one
//! module per aggregate, each holding every use case on it.
//!
//! | Module | Aggregate | Use cases |
//! | --- | --- | --- |
//! | [`timeline`] | the timeline built from the spool | take a line in, catch up, rebuild |
//!
//! A use case takes explicit arguments and returns the [`Request`] that
//! makes it real, or a plain value for a rule. It never performs I/O:
//! `crate::infra::dispatch` carries the request out, and the answer comes
//! back as a [`Message`](crate::core::message::Message).
//!
//! This layer is the specification. Each use case is a function whose doc
//! comment says, in plain words, what can be done and the rules it follows;
//! its tests state those rules one by one. How to write them is in
//! `docs/development.md` ("The use case layer").

pub mod timeline;

/// What a use case asks of the world outside the core, by the aggregate it
/// is about. The use cases' output port: `dispatch` carries each out.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Timeline(timeline::Request),
}

impl From<timeline::Request> for Request {
    fn from(request: timeline::Request) -> Self {
        Self::Timeline(request)
    }
}
