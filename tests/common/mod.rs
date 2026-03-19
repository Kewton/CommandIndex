use assert_cmd::Command;

/// Create a Command for the commandindex binary.
pub fn cmd() -> Command {
    Command::cargo_bin("commandindex").expect("binary should exist")
}
