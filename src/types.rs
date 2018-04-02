use std::fmt::{Display, Formatter, Result as DisplayResult};
#[derive(Debug)]
pub(crate) enum StateMachine {
    Start,
    First {
        file: Vec<u8>,
    },
    Second {
        emojis: String,
        file: Vec<u8>,
    },
    End {
        title: String,
        emojis: String,
        file: Vec<u8>,
    },
}

impl Display for StateMachine {
    fn fmt(&self, f: &mut Formatter) -> DisplayResult {
        match self {
            StateMachine::Start => write!(f, "Send photo: "),
            StateMachine::First { .. } => write!(f, "Send emoji: "),
            StateMachine::Second { .. } => write!(f, "Send title: "),
            StateMachine::End { .. } => write!(f, "Send /publish"),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub(crate) enum ErrorKind {
    // indicates some failure in Hyper, missing network connection, etc.
    #[fail(display = "There was an error fetching the content")]
    Hyper,
}
