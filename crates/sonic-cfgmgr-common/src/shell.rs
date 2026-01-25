//! Shell command execution utilities for cfgmgr daemons.
//!
//! This module provides safe shell command execution with proper quoting
//! to prevent command injection attacks. It mirrors the functionality of
//! the C++ `shellcmd.h` header.
//!
//! # Example
//!
//! ```ignore
//! use sonic_cfgmgr_common::shell::{self, IP_CMD, shellquote};
//!
//! let alias = "Ethernet0";
//! let mtu = "9100";
//! let cmd = format!("{} link set dev {} mtu {}",
//!     IP_CMD, shellquote(alias), shellquote(mtu));
//! let result = shell::exec(&cmd).await?;
//! ```

use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Stdio;
use tokio::process::Command;

use crate::error::{CfgMgrError, CfgMgrResult};

/// Path to the `ip` command for network interface configuration.
pub const IP_CMD: &str = "/sbin/ip";

/// Path to the `bridge` command for bridge/VLAN configuration.
pub const BRIDGE_CMD: &str = "/sbin/bridge";

/// Path to the `brctl` command for legacy bridge control.
pub const BRCTL_CMD: &str = "/sbin/brctl";

/// Path to the `echo` command.
pub const ECHO_CMD: &str = "/bin/echo";

/// Path to the `bash` shell for complex command sequences.
pub const BASH_CMD: &str = "/bin/bash";

/// Path to the `grep` command.
pub const GREP_CMD: &str = "/bin/grep";

/// Path to the `teamd` daemon for LAG management.
pub const TEAMD_CMD: &str = "/usr/bin/teamd";

/// Path to the `teamdctl` control utility for LAG.
pub const TEAMDCTL_CMD: &str = "/usr/bin/teamdctl";

/// Path to the `iptables` command for NAT/firewall rules.
pub const IPTABLES_CMD: &str = "/sbin/iptables";

/// Path to the `conntrack` command for connection tracking.
pub const CONNTRACK_CMD: &str = "/usr/sbin/conntrack";

/// Regex for characters that need escaping in shell double-quotes.
/// Matches: $, `, ", \, and newline
static SHELL_ESCAPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"([$`"\\\n])"#).expect("Invalid regex pattern"));

/// Quotes a string for safe use in shell commands.
///
/// This function wraps the string in double quotes and escapes any
/// characters that have special meaning inside double quotes:
/// - `$` (variable expansion)
/// - `` ` `` (command substitution)
/// - `"` (quote termination)
/// - `\` (escape character)
/// - newline (command termination)
///
/// # Arguments
///
/// * `s` - The string to quote
///
/// # Returns
///
/// A safely quoted string that can be used in shell commands.
///
/// # Example
///
/// ```
/// use sonic_cfgmgr_common::shell::shellquote;
///
/// assert_eq!(shellquote("simple"), "\"simple\"");
/// assert_eq!(shellquote("with$var"), "\"with\\$var\"");
/// assert_eq!(shellquote("with\"quote"), "\"with\\\"quote\"");
/// ```
pub fn shellquote(s: &str) -> String {
    let escaped = SHELL_ESCAPE_RE.replace_all(s, r"\$1");
    format!("\"{}\"", escaped)
}

/// Result of a shell command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// The exit code of the command (0 = success).
    pub exit_code: i32,
    /// The combined stdout output.
    pub stdout: String,
    /// The combined stderr output.
    pub stderr: String,
}

impl ExecResult {
    /// Returns true if the command succeeded (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Returns the combined output (stdout + stderr) for error messages.
    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// Executes a shell command asynchronously.
///
/// This function runs the command through `/bin/sh -c` to support
/// shell features like pipes, redirects, and command chaining.
///
/// # Arguments
///
/// * `cmd` - The command string to execute
///
/// # Returns
///
/// * `Ok(ExecResult)` - The command execution result
/// * `Err(CfgMgrError)` - If the command could not be spawned
///
/// # Example
///
/// ```ignore
/// use sonic_cfgmgr_common::shell;
///
/// let result = shell::exec("/sbin/ip link show").await?;
/// if result.success() {
///     println!("Output: {}", result.stdout);
/// } else {
///     eprintln!("Failed with code {}: {}", result.exit_code, result.stderr);
/// }
/// ```
pub async fn exec(cmd: &str) -> CfgMgrResult<ExecResult> {
    tracing::debug!(command = %cmd, "Executing shell command");

    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| CfgMgrError::ShellExec {
            command: cmd.to_string(),
            source: e,
        })?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    let result = ExecResult {
        exit_code,
        stdout,
        stderr,
    };

    if result.success() {
        tracing::trace!(command = %cmd, exit_code = exit_code, "Command succeeded");
    } else {
        tracing::warn!(
            command = %cmd,
            exit_code = exit_code,
            stderr = %result.stderr,
            "Command failed"
        );
    }

    Ok(result)
}

/// Executes a shell command and throws an error on non-zero exit.
///
/// This is equivalent to the C++ `EXEC_WITH_ERROR_THROW` macro.
///
/// # Arguments
///
/// * `cmd` - The command string to execute
///
/// # Returns
///
/// * `Ok(String)` - The stdout output on success
/// * `Err(CfgMgrError)` - If the command fails or returns non-zero
pub async fn exec_or_throw(cmd: &str) -> CfgMgrResult<String> {
    let result = exec(cmd).await?;
    if result.success() {
        Ok(result.stdout)
    } else {
        Err(CfgMgrError::ShellCommandFailed {
            command: cmd.to_string(),
            exit_code: result.exit_code,
            output: result.combined_output(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shellquote_simple() {
        assert_eq!(shellquote("simple"), "\"simple\"");
        assert_eq!(shellquote("Ethernet0"), "\"Ethernet0\"");
        assert_eq!(shellquote("9100"), "\"9100\"");
    }

    #[test]
    fn test_shellquote_special_chars() {
        // Dollar sign (variable expansion)
        assert_eq!(shellquote("$HOME"), "\"\\$HOME\"");

        // Backtick (command substitution)
        assert_eq!(shellquote("`whoami`"), "\"\\`whoami\\`\"");

        // Double quote
        assert_eq!(shellquote("say \"hello\""), "\"say \\\"hello\\\"\"");

        // Backslash
        assert_eq!(shellquote("path\\to"), "\"path\\\\to\"");

        // Newline
        assert_eq!(shellquote("line1\nline2"), "\"line1\\\nline2\"");
    }

    #[test]
    fn test_shellquote_combined() {
        // Multiple special characters
        let input = "$USER says \"hello\" via `echo`";
        let expected = "\"\\$USER says \\\"hello\\\" via \\`echo\\`\"";
        assert_eq!(shellquote(input), expected);
    }

    #[test]
    fn test_shellquote_empty() {
        assert_eq!(shellquote(""), "\"\"");
    }

    #[test]
    fn test_exec_result_success() {
        let result = ExecResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: "".to_string(),
        };
        assert!(result.success());
        assert_eq!(result.combined_output(), "output");
    }

    #[test]
    fn test_exec_result_failure() {
        let result = ExecResult {
            exit_code: 1,
            stdout: "".to_string(),
            stderr: "error message".to_string(),
        };
        assert!(!result.success());
        assert_eq!(result.combined_output(), "error message");
    }

    #[test]
    fn test_exec_result_combined() {
        let result = ExecResult {
            exit_code: 0,
            stdout: "stdout".to_string(),
            stderr: "stderr".to_string(),
        };
        assert_eq!(result.combined_output(), "stdout\nstderr");
    }

    #[tokio::test]
    async fn test_exec_echo() {
        let result = exec("echo hello").await.unwrap();
        assert!(result.success());
        assert_eq!(result.stdout, "hello");
    }

    #[tokio::test]
    async fn test_exec_failure() {
        let result = exec("exit 42").await.unwrap();
        assert!(!result.success());
        assert_eq!(result.exit_code, 42);
    }

    #[tokio::test]
    async fn test_exec_or_throw_success() {
        let output = exec_or_throw("echo success").await.unwrap();
        assert_eq!(output, "success");
    }

    #[tokio::test]
    async fn test_exec_or_throw_failure() {
        let result = exec_or_throw("exit 1").await;
        assert!(result.is_err());
        match result {
            Err(CfgMgrError::ShellCommandFailed { exit_code, .. }) => {
                assert_eq!(exit_code, 1);
            }
            _ => panic!("Expected ShellCommandFailed error"),
        }
    }
}
