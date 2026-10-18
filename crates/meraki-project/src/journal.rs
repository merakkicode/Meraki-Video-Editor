use crate::{load, save, Project, ProjectResult};
use std::path::{Path, PathBuf};

/// Journals use the same bounded, validated, atomic format as explicit project saves.
pub fn write_journal(path: &Path, project: &Project) -> ProjectResult<()> {
    std::fs::create_dir_all(path.parent().ok_or("Diretório de recuperação ausente")?)
        .map_err(|e| e.to_string())?;
    save(path, project)
}
pub fn journal_path(directory: &Path, name: &str) -> ProjectResult<PathBuf> {
    if !name.starts_with("recovery-")
        || !name.ends_with(".meraki")
        || name.contains(['/', '\\', ':'])
        || name.len() > 128
    {
        return Err("Nome de recuperação inválido".into());
    }
    Ok(directory.join(name))
}
pub fn recover_journal(directory: &Path, name: &str) -> ProjectResult<Project> {
    load(&journal_path(directory, name)?)
}
pub fn list_journals(directory: &Path) -> ProjectResult<Vec<String>> {
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file() && journal_path(directory, &name).is_ok() {
            names.push(name);
        }
    }
    names.sort();
    names.reverse();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_survives_terminated_writer_process() {
        if let Some(directory) = std::env::var_os("MERAKI_TEST_CRASH_JOURNAL") {
            let path = PathBuf::from(directory).join("recovery-crash.meraki");
            let mut project = crate::tests::sample();
            project.timeline.name = "Edição antes da falha".into();
            write_journal(&path, &project).unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "journal::tests::recovery_survives_terminated_writer_process",
            ])
            .env("MERAKI_TEST_CRASH_JOURNAL", directory.path())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let path = directory.path().join("recovery-crash.meraki");
        while !path.is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let restored = recover_journal(directory.path(), "recovery-crash.meraki").unwrap();
        assert_eq!(restored.timeline.name, "Edição antes da falha");
        restored.validate().unwrap();
    }
    #[test]
    fn journal_roundtrip_atomic_replacement_and_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = crate::tests::sample();
        p.assets[0].path = dir.path().join("sample.mp4");
        let path = journal_path(dir.path(), "recovery-1.meraki").unwrap();
        write_journal(&path, &p).unwrap();
        assert_eq!(recover_journal(dir.path(), "recovery-1.meraki").unwrap(), p);
        let mut edited = p.clone();
        edited.timeline.name = "Recuperado".into();
        write_journal(&path, &edited).unwrap();
        assert_eq!(load(&path).unwrap(), edited);
        let mut invalid = edited.clone();
        invalid.schema_version = 999;
        assert!(write_journal(&path, &invalid).is_err());
        assert_eq!(load(&path).unwrap(), edited);
        assert_eq!(list_journals(dir.path()).unwrap().len(), 1);
        assert!(journal_path(dir.path(), "recovery-../../outside.meraki").is_err());
        std::fs::write(&path, b"{broken").unwrap();
        assert!(load(&path).is_err());
    }
}
