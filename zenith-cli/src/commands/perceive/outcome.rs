//! Result types shared by the `zenith perceive` subcommands.

/// A `zenith perceive` failure: the message and the exit code.
#[derive(Debug)]
pub struct PerceiveCmdErr {
    /// Human text. Lines of the form `error[<code>]: <message>` keep their code.
    pub message: String,
    /// Process exit code.
    pub exit_code: u8,
}

/// A `zenith perceive` result: stdout text and the exit code.
#[derive(Debug)]
pub struct PerceiveOutcome {
    /// Text to print on stdout.
    pub stdout: String,
    /// Process exit code.
    pub exit_code: u8,
}
