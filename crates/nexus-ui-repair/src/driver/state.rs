//! 5-state scout machine plus Report terminal action. See v1.1 §2.

/// The scout's state machine.
///
/// `Enumerate → Plan → Act → Observe → Classify → Report`. After
/// `Report`, [`DriverState::next`] returns `None`, signaling that the
/// per-element loop has completed and the driver should advance to the
/// next element (or finish the page if no elements remain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverState {
    Enumerate,
    Plan,
    Act,
    Observe,
    Classify,
    Report,
}

impl DriverState {
    /// Successor state, or `None` if this is the terminal state.
    pub fn next(&self) -> Option<DriverState> {
        match self {
            DriverState::Enumerate => Some(DriverState::Plan),
            DriverState::Plan => Some(DriverState::Act),
            DriverState::Act => Some(DriverState::Observe),
            DriverState::Observe => Some(DriverState::Classify),
            DriverState::Classify => Some(DriverState::Report),
            DriverState::Report => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_to_terminal() {
        let mut s = Some(DriverState::Enumerate);
        let mut visited = Vec::new();
        while let Some(current) = s {
            visited.push(current);
            s = current.next();
        }
        assert_eq!(
            visited,
            vec![
                DriverState::Enumerate,
                DriverState::Plan,
                DriverState::Act,
                DriverState::Observe,
                DriverState::Classify,
                DriverState::Report,
            ]
        );
    }
}
