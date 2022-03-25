//! The type that stores testing configuration.
//!
//! Use the code in this module to tune testing behaviour.

#[cfg(feature = "clap")] pub mod clap;

use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fmt;
use tempfile::NamedTempFile;

const DEFAULT_MAX_OUTPUT_CONTEXT_LINE_COUNT: usize = 10;

/// The configuration of the test runner.
#[derive(Clone, Debug)]
pub struct Config
{
    /// A list of file extensions which contain tests.
    pub supported_file_extensions: Vec<String>,
    /// Paths to tests or folders containing tests.
    pub test_paths: Vec<PathBuf>,
    /// Constants that tests can refer to via `@<name>` syntax.
    pub constants: HashMap<String, String>,
    /// A function which used to dynamically lookup variables.
    ///
    /// The default variable lookup can be found at `Config::DEFAULT_VARIABLE_LOOKUP`.
    ///
    /// In your own custom variable lookups, most of the time you will want to
    /// include a fallback call to `Config::DEFAULT_VARIABLE_LOOKUP`.
    pub variable_lookup: VariableLookup,
    /// Whether temporary files generated by the tests should be
    /// cleaned up, where possible.
    ///
    /// This includes temporary files created by using `@tempfile`
    /// variables.
    pub cleanup_temporary_files: bool,
    /// Export all generated test artifacts to the specified directory.
    pub save_artifacts_to_directory: Option<PathBuf>,
    /// Whether verbose information about resolved variables should be printed to stderr.
    pub dump_variable_resolution: bool,
    /// If set, debug output should be truncated to this many number of
    /// context lines.
    pub truncate_output_context_to_number_of_lines: Option<usize>,
    /// A list of extra directory paths that should be included in the `$PATH` when
    /// executing processes specified inside the tests.
    pub extra_executable_search_paths: Vec<PathBuf>,
    /// Whether messages on the standard error streams emitted during test runs
    /// should always be shown.
    pub always_show_stderr: bool,
    /// Which shell to use (defaults to 'bash').
    pub shell: String,
    /// List of environment variables to be used on test invocation.
    pub env_variables: HashMap<String, String>,
}

/// A function which can dynamically define newly used variables in a test.
#[derive(Clone)]
pub struct VariableLookup(fn(&str) -> Option<String>);

impl Config
{
    /// The default variable lookup function.
    ///
    /// The supported variables are:
    ///
    /// * Any variable containing the string `"tempfile"`
    ///   * Each distinct variable will be resolved to a distinct temporary file path.
    pub const DEFAULT_VARIABLE_LOOKUP: VariableLookup = VariableLookup(|v| {
        if v.contains("tempfile") {
            let temp_file = NamedTempFile::new().expect("failed to create a temporary file");
            Some(temp_file.into_temp_path().to_str().expect("temp file path is not utf-8").to_owned())
        } else {
            None
        }
    });

    /// Marks a file extension as supported by the runner.
    ///
    /// We only attempt to run tests for files within the extension
    /// whitelist.
    pub fn add_extension<S>(&mut self, ext: S) where S: AsRef<str> {
        self.supported_file_extensions.push(ext.as_ref().to_owned())
    }

    /// Marks multiple file extensions as supported by the running.
    pub fn add_extensions(&mut self, extensions: &[&str]) {
        self.supported_file_extensions.extend(extensions.iter().map(|s| s.to_string()));
    }

    /// Adds a search path to the test runner.
    ///
    /// We will recurse through the path to find tests.
    pub fn add_search_path<P>(&mut self, path: P) where P: Into<String> {
        self.test_paths.push(PathBuf::from(path.into()).canonicalize().unwrap());
    }

    /// Adds an extra executable directory to the OS `$PATH` when executing tests.
    pub fn add_executable_search_path<P>(&mut self, path: P) where P: AsRef<Path> {
        self.extra_executable_search_paths.push(path.as_ref().to_owned())
    }

    /// Gets an iterator over all test search directories.
    pub fn test_search_directories(&self) -> impl Iterator<Item=&Path> {
        self.test_paths.iter().filter(|p| {
            println!("test path file name: {:?}", p.file_name());
            p.is_dir()
        }).map(PathBuf::as_ref)
    }

    /// Checks if a given extension will have tests run on it
    pub fn is_extension_supported(&self, extension: &str) -> bool {
        self.supported_file_extensions.iter().
            find(|ext| &ext[..] == extension).is_some()
    }

    /// Looks up a variable.
    pub fn lookup_variable<'a>(&self,
                           name: &str,
                           variables: &'a mut HashMap<String, String>)
        -> &'a str {
        if !variables.contains_key(name) {
            match self.variable_lookup.0(name) {
                Some(initial_value) => {
                    variables.insert(name.to_owned(), initial_value.clone());
                },
                None => (),
            }
        }

        variables.get(name).expect(&format!("no variable with the name '{}' exists", name))
    }
}

impl Default for Config
{
    fn default() -> Self {
        let mut extra_executable_search_paths = Vec::new();

        // Always inject the current directory of the executable into the PATH so
        // that lit can be used manually inside the test if desired.
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                extra_executable_search_paths.push(parent.to_owned());
            }
        }

        Config {
            supported_file_extensions: Vec::new(),
            test_paths: Vec::new(),
            constants: HashMap::new(),
            variable_lookup: Config::DEFAULT_VARIABLE_LOOKUP,
            cleanup_temporary_files: true,
            save_artifacts_to_directory: None,
            dump_variable_resolution: false,
            always_show_stderr: false,
            truncate_output_context_to_number_of_lines: Some(DEFAULT_MAX_OUTPUT_CONTEXT_LINE_COUNT),
            extra_executable_search_paths,
            shell: "bash".to_string(),
            env_variables: HashMap::default(),
        }
    }
}

impl fmt::Debug for VariableLookup {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        "<function>".fmt(fmt)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lookup_variable_works_correctly() {
        let config = Config {
            variable_lookup: VariableLookup(|v| {
                if v.contains("tempfile") { Some(format!("/tmp/temp-{}", v.as_bytes().as_ptr() as usize)) } else { None }
            }),
            constants: vec![("name".to_owned(), "bob".to_owned())].into_iter().collect(),
            ..Config::default()
        };
        let mut variables = config.constants.clone();

        // Can lookup constants
        assert_eq!("bob", config.lookup_variable("name", &mut variables),
                   "cannot lookup constants by name");
        let first_temp = config.lookup_variable("first_tempfile", &mut variables).to_owned();
        let second_temp = config.lookup_variable("second_tempfile", &mut variables).to_owned();

        assert!(first_temp != second_temp,
                "different temporary paths should be different");

        assert_eq!(first_temp,
                   config.lookup_variable("first_tempfile", &mut variables),
                   "first temp has changed its value");

        assert_eq!(second_temp,
                   config.lookup_variable("second_tempfile", &mut variables),
                   "second temp has changed its value");
    }
}

