mod session {
    #[derive(Debug)]
    pub enum Error { InvalidDirectory(std::path::PathBuf) }
}
mod directory;
