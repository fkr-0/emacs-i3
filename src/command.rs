use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeAction {
    Grow,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeAxis {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Focus(Direction),
    Move(Direction),
    Resize {
        action: ResizeAction,
        axis: ResizeAxis,
        arguments: Vec<String>,
    },
    LayoutToggleSplit,
    Split(SplitDirection),
    Kill,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    pub raw: String,
    pub operation: Operation,
}

impl ParsedCommand {
    pub fn parse(command: &str) -> Self {
        let tokens = command.split_whitespace().collect::<Vec<_>>();
        let operation = match tokens.as_slice() {
            ["focus", direction] => Direction::parse(direction)
                .map(Operation::Focus)
                .unwrap_or(Operation::Unknown),
            ["move", direction] => Direction::parse(direction)
                .map(Operation::Move)
                .unwrap_or(Operation::Unknown),
            ["resize", action, axis, rest @ ..] => {
                let action = match *action {
                    "grow" => Some(ResizeAction::Grow),
                    "shrink" => Some(ResizeAction::Shrink),
                    _ => None,
                };
                let axis = match *axis {
                    "width" => Some(ResizeAxis::Width),
                    "height" => Some(ResizeAxis::Height),
                    _ => None,
                };
                match (action, axis) {
                    (Some(action), Some(axis)) => Operation::Resize {
                        action,
                        axis,
                        arguments: rest.iter().map(|value| (*value).to_owned()).collect(),
                    },
                    _ => Operation::Unknown,
                }
            }
            ["layout", "toggle", "split"] => Operation::LayoutToggleSplit,
            ["split", "h"] | ["split", "horizontal"] => {
                Operation::Split(SplitDirection::Horizontal)
            }
            ["split", "v"] | ["split", "vertical"] => Operation::Split(SplitDirection::Vertical),
            ["kill"] => Operation::Kill,
            _ => Operation::Unknown,
        };

        Self {
            raw: command.to_owned(),
            operation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_command_family() {
        assert_eq!(
            ParsedCommand::parse("focus left").operation,
            Operation::Focus(Direction::Left)
        );
        assert_eq!(
            ParsedCommand::parse("move down").operation,
            Operation::Move(Direction::Down)
        );
        assert_eq!(
            ParsedCommand::parse("resize grow width 10 px").operation,
            Operation::Resize {
                action: ResizeAction::Grow,
                axis: ResizeAxis::Width,
                arguments: vec!["10".to_owned(), "px".to_owned()],
            }
        );
        assert_eq!(
            ParsedCommand::parse("layout toggle split").operation,
            Operation::LayoutToggleSplit
        );
        assert_eq!(
            ParsedCommand::parse("split v").operation,
            Operation::Split(SplitDirection::Vertical)
        );
        assert_eq!(ParsedCommand::parse("kill").operation, Operation::Kill);
    }

    #[test]
    fn preserves_unknown_commands_for_i3_fallback() {
        let parsed = ParsedCommand::parse("workspace next");
        assert_eq!(parsed.operation, Operation::Unknown);
        assert_eq!(parsed.raw, "workspace next");
    }
}
