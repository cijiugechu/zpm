use clipanion::cli;

#[cli::command]
#[cli::path("switch")]
#[derive(Debug)]
pub struct VersionCommand {
    #[cli::option("--version")]
    version: bool,
}

impl VersionCommand {
    pub async fn execute(&self) {
        println!("{}", self.cli_environment.info.version);
    }
}
