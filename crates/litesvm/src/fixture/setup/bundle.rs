use {
    super::SetupError,
    crate::fixture::Pubkey,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

pub(super) struct BundledProgram {
    pub(super) id: Pubkey,
    pub(super) path: PathBuf,
}

pub(super) fn discover_program_bundle(
    primary_program: &Path,
    primary_id: Pubkey,
) -> Result<Vec<BundledProgram>, SetupError> {
    let Some(deploy) = primary_program.parent() else {
        return Ok(Vec::new());
    };
    let entries = fs::read_dir(deploy).map_err(|source| SetupError::ReadDeployDirectory {
        path: deploy.to_path_buf(),
        source,
    })?;
    let mut programs = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| SetupError::ReadDeployDirectory {
                path: deploy.to_path_buf(),
                source,
            })?
            .path();
        if path != primary_program && path.extension().is_some_and(|extension| extension == "so") {
            programs.push(path);
        }
    }
    programs.sort();

    let mut bundle = Vec::new();
    for path in programs {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let keypair = path.with_file_name(format!("{stem}-keypair.json"));
        if !keypair.is_file() {
            continue;
        }
        let id = read_program_id(&keypair)?;
        if id != primary_id {
            bundle.push(BundledProgram { id, path });
        }
    }
    Ok(bundle)
}

fn read_program_id(keypair: &Path) -> Result<Pubkey, SetupError> {
    let bytes = fs::read(keypair).map_err(|source| SetupError::ReadProgramKeypair {
        path: keypair.to_path_buf(),
        source,
    })?;
    let secret = serde_json::from_slice::<Vec<u8>>(&bytes).map_err(|source| {
        SetupError::InvalidProgramKeypair {
            path: keypair.to_path_buf(),
            reason: source.to_string(),
        }
    })?;
    let secret: [u8; 64] =
        secret
            .try_into()
            .map_err(|secret: Vec<u8>| SetupError::InvalidProgramKeypair {
                path: keypair.to_path_buf(),
                reason: format!("expected 64 bytes, found {}", secret.len()),
            })?;
    Ok(Pubkey::new_from_array(
        secret[32..].try_into().expect("32-byte public key"),
    ))
}
