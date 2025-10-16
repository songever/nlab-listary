use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Failed to open URL: {0}")]
    OpenError(String),
}

pub fn open_url(url: &str) -> Result<(), BrowserError> {
    // Use system default browser via open crate as fallback
    match open::that(url) {
        Ok(()) => Ok(()),
        Err(e) => Err(BrowserError::OpenError(format!(
            "Failed to open URL '{}': {}",
            url, e
        ))),
    }
}
