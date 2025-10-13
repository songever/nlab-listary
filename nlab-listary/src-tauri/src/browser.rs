use std::process::ExitStatus;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("No browser commands available for this URL")]
    NoCommandsAvailable,

    #[error("Failed to execute browser command")]
    CommandFailed(#[from] std::io::Error),

    #[error("Browser command exited with non-zero status: {0}")]
    CommandExitedWithError(ExitStatus),
}

pub fn open_url(url: &str) -> Result<(), BrowserError> {
    let mut commands = open::commands(url);

    let status = commands
        .first_mut() // 使用 first_mut() 获取可变引用
        .ok_or(BrowserError::NoCommandsAvailable)?
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(BrowserError::CommandExitedWithError(status))
    }
}
