pub mod read_aspect_file;
pub mod read_config;
pub mod read_prompt_template;
pub mod resolve_project;
// Consumed by the driver-checkpoint and CLI-lifecycle slices that follow.
#[allow(dead_code)]
pub mod run_storage;
pub mod state_storage;
pub mod write_config;

pub use read_aspect_file::ReadAspectFile;
pub use read_config::ReadConfig;
pub use read_prompt_template::ReadPromptTemplate;
pub use resolve_project::ResolveProject;
pub use state_storage::EnsureProject;
pub use write_config::WriteConfig;

pub struct AppContext;
