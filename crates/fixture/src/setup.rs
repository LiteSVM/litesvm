use {
    crate::{backend::Backend, Ctx, Pubkey},
    std::{
        env,
        error::Error,
        fmt, fs,
        path::{Path, PathBuf},
    },
};

mod bundle;

use bundle::discover_program_bundle;

/// Environment variable naming the compiled program artifact to load. A test
/// runner (or [`crate::parallax_test`]) may set it to a freshly built program;
/// left unset, the artifact is discovered under an ancestor `target/deploy`.
pub const PROGRAM_PATH_ENV: &str = "PARALLAX_PROGRAM_PATH";

/// World setup: which program artifact to load and its runtime limits.
///
/// Created by [`Ctx::builder`].
#[must_use = "call .build() to construct the Ctx"]
pub struct CtxBuilder {
    pub(super) program_id: Pubkey,
    pub(super) compute_unit_limit: Option<u64>,
    pub(super) program_path: Option<PathBuf>,
    pub(super) crate_name: Option<String>,
    pub(super) program_elf: Option<Vec<u8>>,
    pub(super) rpc_url: Option<String>,
    pub(super) project_dir: Option<String>,
}

impl CtxBuilder {
    pub(crate) fn new(program_id: Pubkey) -> Self {
        Self {
            program_id,
            compute_unit_limit: None,
            program_path: None,
            crate_name: None,
            program_elf: None,
            rpc_url: None,
            project_dir: None,
        }
    }

    /// Set the transaction compute-unit limit for this world.
    pub fn compute_unit_limit(mut self, limit: u64) -> Self {
        self.compute_unit_limit = Some(limit);
        self
    }

    /// Set the RPC endpoint that [`Dump`](crate::fixture::Dump) fixtures fetch
    /// from on a store miss. This is code-only and set once; unset, it defaults
    /// to the public mainnet-beta RPC. There is deliberately no environment
    /// override — the endpoint lives in the test, not the ambient environment.
    pub fn rpc(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    /// Set the project directory whose committed `.parallax/` store the world's
    /// [`Dump`](crate::fixture::Dump) fixtures read and write. The
    /// `#[parallax_test]` macro passes `CARGO_MANIFEST_DIR` here; unset, the
    /// store is resolved from `CARGO_MANIFEST_DIR` or the nearest ancestor
    /// `Cargo.toml`.
    pub fn project_dir(mut self, dir: impl Into<String>) -> Self {
        self.project_dir = Some(dir.into());
        self
    }

    /// Load an explicit program artifact instead of discovering one.
    pub fn program_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.program_path = Some(path.into());
        self
    }

    /// Prefer `target/deploy/{crate_name}.so` (with `-` mapped to `_`) during
    /// discovery, so tests resolve their own program in a workspace that
    /// builds several. `#[parallax_test]` passes `env!("CARGO_PKG_NAME")`.
    pub fn crate_name(mut self, name: impl Into<String>) -> Self {
        self.crate_name = Some(name.into());
        self
    }

    /// Load the primary program from in-memory ELF bytes, skipping on-disk
    /// artifact discovery and the sibling CPI-bundle scan.
    ///
    /// Use when the caller already holds the compiled program, such as a test
    /// embedding the artifact with `include_bytes!`. When set, [`Self::build`] loads exactly this ELF
    /// under the id passed to [`Ctx::builder`](crate::Ctx::builder) and
    /// ignores [`Self::program_path`], [`Self::crate_name`], and
    /// [`PROGRAM_PATH_ENV`]. Additional programs are added afterwards with
    /// [`Ctx::preload_program`](crate::Ctx::preload_program).
    ///
    /// Empty bytes are equivalent to [`Self::no_program`].
    pub fn program_bytes(mut self, elf: impl Into<Vec<u8>>) -> Self {
        self.program_elf = Some(elf.into());
        self
    }

    /// Build a world with no primary program, loading only the runtime's
    /// built-in programs (the system program and the SPL Token, Token-2022, and
    /// Associated Token programs).
    ///
    /// Use for tests that exercise only those built-ins — a bare token transfer,
    /// an account layout — without a program of their own, or when every program
    /// under test is added afterwards with
    /// [`Ctx::preload_program`](crate::Ctx::preload_program). Like
    /// [`Self::program_bytes`], this skips on-disk artifact discovery and the
    /// sibling CPI-bundle scan; the id passed to
    /// [`Ctx::builder`](crate::Ctx::builder) still names the world for PDA
    /// derivation.
    pub fn no_program(mut self) -> Self {
        self.program_elf = Some(Vec::new());
        self
    }

    /// Load the program and start the world.
    pub fn build(self) -> Result<Ctx, SetupError> {
        let CtxBuilder {
            program_id,
            compute_unit_limit,
            program_path,
            crate_name,
            program_elf,
            rpc_url,
            project_dir,
        } = self;
        let rpc_url = rpc_url.unwrap_or_else(|| crate::dump::DEFAULT_RPC_URL.to_string());

        if let Some(elf) = program_elf {
            let mut backend = Backend::new();
            if let Some(limit) = compute_unit_limit {
                backend.set_compute_unit_limit(limit);
            }
            // No ELF (an empty program_bytes, or no_program) means a world with
            // just the runtime's built-ins; load nothing.
            if !elf.is_empty() {
                backend.load_program(&program_id, &elf);
            }
            return Ok(Ctx::from_parts(
                backend,
                program_id,
                PathBuf::new(),
                rpc_url,
                project_dir,
            ));
        }
        let path = match program_path {
            Some(path) => path,
            None => resolve_program_path(crate_name.as_deref())?,
        };
        let elf = fs::read(&path).map_err(|source| SetupError::ReadProgram {
            path: path.clone(),
            source,
        })?;
        let mut backend = Backend::new();
        if let Some(limit) = compute_unit_limit {
            backend.set_compute_unit_limit(limit);
        }
        backend.load_program(&program_id, &elf);
        for program in discover_program_bundle(&path, program_id)? {
            let elf = fs::read(&program.path).map_err(|source| SetupError::ReadBundledProgram {
                path: program.path.clone(),
                source,
            })?;
            backend.load_program(&program.id, &elf);
        }
        Ok(Ctx::from_parts(
            backend,
            program_id,
            path,
            rpc_url,
            project_dir,
        ))
    }
}

/// Resolve the compiled program path: the [`PROGRAM_PATH_ENV`] override, then
/// discovery from the current directory.
fn resolve_program_path(crate_name: Option<&str>) -> Result<PathBuf, SetupError> {
    if let Some(path) = configured_program_path()? {
        return Ok(path);
    }
    let current_dir = env::current_dir().map_err(SetupError::CurrentDirectory)?;
    resolve_program_path_from_named(&current_dir, crate_name)
}

fn configured_program_path() -> Result<Option<PathBuf>, SetupError> {
    let Some(path) = env::var_os(PROGRAM_PATH_ENV) else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    if path.is_file() {
        return Ok(Some(path));
    }
    Err(SetupError::ConfiguredProgramMissing {
        env: PROGRAM_PATH_ENV,
        path,
    })
}

pub(super) fn resolve_program_path_from_named(
    start: &Path,
    crate_name: Option<&str>,
) -> Result<PathBuf, SetupError> {
    let artifact = crate_name.map(|name| format!("{}.so", name.replace('-', "_")));
    let mut checked = Vec::new();
    for ancestor in start.ancestors() {
        let deploy = ancestor.join("target/deploy");
        checked.push(deploy.clone());
        if let Some(ref artifact) = artifact {
            let path = deploy.join(artifact);
            if path.is_file() {
                return Ok(path);
            }
        }
        let mut programs = match fs::read_dir(&deploy) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "so"))
                .collect::<Vec<_>>(),
            Err(_) => continue,
        };
        programs.sort();
        if programs.len() == 1 {
            return Ok(programs.remove(0));
        }
        if programs.len() > 1 {
            return Err(SetupError::AmbiguousPrograms { deploy, programs });
        }
    }

    Err(SetupError::ProgramNotFound {
        start: start.to_path_buf(),
        checked,
    })
}

/// Failure to locate or load a compiled program.
#[derive(Debug)]
#[non_exhaustive]
pub enum SetupError {
    /// A configured program path no longer exists.
    ConfiguredProgramMissing {
        /// The environment variable that supplied the path —
        /// [`PROGRAM_PATH_ENV`] natively, or a downstream bridge's own
        /// variable, so the diagnostic names the one the user actually set.
        env: &'static str,
        /// Missing path supplied through `env`.
        path: PathBuf,
    },
    /// The current working directory could not be read.
    CurrentDirectory(std::io::Error),
    /// No unambiguous program was found under an ancestor `target/deploy`.
    ProgramNotFound {
        /// Directory from which ancestor discovery began.
        start: PathBuf,
        /// Candidate deploy directories that were inspected.
        checked: Vec<PathBuf>,
    },
    /// More than one program artifact exists in the closest deploy directory.
    AmbiguousPrograms {
        /// Deploy directory containing multiple candidates.
        deploy: PathBuf,
        /// Candidate program artifacts in that directory.
        programs: Vec<PathBuf>,
    },
    /// The selected program artifact could not be read.
    ReadProgram {
        /// Program artifact that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The directory containing the primary program could not be inspected.
    ReadDeployDirectory {
        /// Deploy directory that could not be inspected.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// An automatically discovered CPI program could not be read.
    ReadBundledProgram {
        /// Bundled program artifact that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A deployed program's keypair file could not be read.
    ReadProgramKeypair {
        /// Program keypair that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// A deployed program's keypair did not contain a valid 64-byte keypair.
    InvalidProgramKeypair {
        /// Invalid program keypair file.
        path: PathBuf,
        /// Why the keypair could not identify a program.
        reason: String,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredProgramMissing { env, path } => write!(
                formatter,
                "{env} points to missing program artifact {}",
                path.display()
            ),
            Self::CurrentDirectory(source) => {
                write!(
                    formatter,
                    "could not resolve the current project directory: {source}"
                )
            }
            Self::ProgramNotFound { start, checked } => {
                write!(
                    formatter,
                    "could not find one compiled program from {}; build a program into \
                     target/deploy or set {PROGRAM_PATH_ENV}",
                    start.display()
                )?;
                if !checked.is_empty() {
                    write!(formatter, " (checked")?;
                    for path in checked {
                        write!(formatter, " {}", path.display())?;
                    }
                    write!(formatter, ")")?;
                }
                Ok(())
            }
            Self::AmbiguousPrograms { deploy, programs } => {
                write!(
                    formatter,
                    "found multiple program artifacts in {}; set {PROGRAM_PATH_ENV} to the \
                     intended artifact:",
                    deploy.display()
                )?;
                for path in programs {
                    write!(formatter, " {}", path.display())?;
                }
                Ok(())
            }
            Self::ReadProgram { path, source } => write!(
                formatter,
                "could not read program artifact {}: {source}",
                path.display()
            ),
            Self::ReadDeployDirectory { path, source } => write!(
                formatter,
                "could not inspect program bundle in {}: {source}",
                path.display()
            ),
            Self::ReadBundledProgram { path, source } => write!(
                formatter,
                "could not read bundled CPI program {}: {source}",
                path.display()
            ),
            Self::ReadProgramKeypair { path, source } => write!(
                formatter,
                "could not read program keypair {}: {source}",
                path.display()
            ),
            Self::InvalidProgramKeypair { path, reason } => write!(
                formatter,
                "invalid program keypair {}: {reason}",
                path.display()
            ),
        }
    }
}

impl Error for SetupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentDirectory(source)
            | Self::ReadProgram { source, .. }
            | Self::ReadDeployDirectory { source, .. }
            | Self::ReadBundledProgram { source, .. }
            | Self::ReadProgramKeypair { source, .. } => Some(source),
            Self::ConfiguredProgramMissing { .. }
            | Self::ProgramNotFound { .. }
            | Self::AmbiguousPrograms { .. }
            | Self::InvalidProgramKeypair { .. } => None,
        }
    }
}
