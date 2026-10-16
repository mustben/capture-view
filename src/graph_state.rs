//! Preserve successful-but-intermediate HRESULT instead of collapsing it to Result<()>.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Transitioning,
    Stopped,
    Other(i32),
}
pub fn classify(hr: i32, state: i32) -> Result<State, i32> {
    match hr {
        n if n < 0 => Err(n),
        0x00040237 => Ok(State::Transitioning), // VFW_S_STATE_INTERMEDIATE
        0 => Ok(match state {
            2 => State::Running,
            0 => State::Stopped,
            n => State::Other(n),
        }),
        _ => Ok(State::Other(state)), // Unknown success is not confirmed running.
    }
}
/// Caller supplies elapsed deadline to keep polling deterministic and hardware-free.
pub fn startup_confirmed(state: &State, expired: bool) -> Result<bool, &'static str> {
    if *state == State::Running {
        Ok(true)
    } else if expired {
        Err("Graph startup confirmation timed out after 10 s")
    } else {
        Ok(false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_and_deadline_are_distinct() {
        assert_eq!(startup_confirmed(&State::Transitioning, false), Ok(false));
        assert_eq!(startup_confirmed(&State::Stopped, false), Ok(false));
        assert_eq!(startup_confirmed(&State::Running, false), Ok(true));
        assert!(startup_confirmed(&State::Transitioning, true).is_err());
    }
    #[test]
    fn requires_s_ok_and_running() {
        assert_eq!(classify(0x40237, 2), Ok(State::Transitioning));
        assert_eq!(classify(0, 2), Ok(State::Running));
        assert_eq!(classify(0, 0), Ok(State::Stopped));
        assert_eq!(classify(-1, 2), Err(-1));
        assert_ne!(classify(1, 2), Ok(State::Running));
    }
}
