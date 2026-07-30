use super::{InternalSkill, NameConflict, Source, Workspace, WorkspaceError, MAX_SCAN_DEPTH};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecyclePreview {
    pub action: String,
    pub id: String,
    pub name: String,
    pub source: String,
    pub destination_source: Option<String>,
    pub destination: Option<String>,
    pub directory_revision: String,
    pub conflict: Option<NameConflict>,
    pub can_apply: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleResult {
    pub ok: bool,
    pub id: String,
    pub source: String,
    pub destination: String,
    pub directory_revision: String,
    pub restart_recommended: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSkillResult {
    pub ok: bool,
    pub deleted_name: String,
    pub restart_recommended: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleAction {
    Disable,
    Enable,
    Archive,
    Restore,
    Delete,
}

impl LifecycleAction {
    fn parse(value: &str) -> Result<Self, WorkspaceError> {
        match value {
            "disable" => Ok(Self::Disable),
            "enable" => Ok(Self::Enable),
            "archive" => Ok(Self::Archive),
            "restore" => Ok(Self::Restore),
            "delete" => Ok(Self::Delete),
            _ => Err(WorkspaceError::InvalidLifecycleAction),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Enable => "enable",
            Self::Archive => "archive",
            Self::Restore => "restore",
            Self::Delete => "delete",
        }
    }

    fn destination_source(self, source: Source) -> Result<Option<Source>, WorkspaceError> {
        match (self, source) {
            (Self::Disable, Source::Personal) => Ok(Some(Source::Disabled)),
            (Self::Enable, Source::Disabled) => Ok(Some(Source::Personal)),
            (Self::Archive, Source::Personal | Source::Disabled) => Ok(Some(Source::Archive)),
            (Self::Restore, Source::Archive) => Ok(Some(Source::Personal)),
            (Self::Delete, Source::Archive) => Ok(None),
            _ => Err(WorkspaceError::LifecycleNotAllowed),
        }
    }
}

impl Workspace {
    pub fn preview_skill_lifecycle(
        &self,
        id: &str,
        action: &str,
    ) -> Result<LifecyclePreview, WorkspaceError> {
        let action = LifecycleAction::parse(action)?;
        let skill = self.find_skill(id)?;
        let destination_source = action.destination_source(skill.source)?;
        let source_directory = validated_skill_directory(&skill)?;
        let directory_revision = directory_revision(&source_directory)?;
        let destination = destination_source.map(|source| {
            self.root_for_source(source)
                .join(&skill.summary.directory_name)
        });
        let conflict = destination.as_ref().and_then(|path| {
            fs::symlink_metadata(path).ok().map(|_| NameConflict {
                source: destination_source
                    .expect("destination path has a source")
                    .label()
                    .to_string(),
                path: path.display().to_string(),
            })
        });
        Ok(LifecyclePreview {
            action: action.label().into(),
            id: skill.summary.id,
            name: skill.summary.name,
            source: skill.source.label().into(),
            destination_source: destination_source.map(|source| source.label().into()),
            destination: destination.map(|path| path.display().to_string()),
            directory_revision,
            can_apply: conflict.is_none(),
            conflict,
        })
    }

    pub fn apply_skill_lifecycle(
        &self,
        id: &str,
        action: &str,
        expected_directory_revision: &str,
    ) -> Result<LifecycleResult, WorkspaceError> {
        let action = LifecycleAction::parse(action)?;
        if action == LifecycleAction::Delete {
            return Err(WorkspaceError::LifecycleNotAllowed);
        }
        let preview = self.preview_skill_lifecycle(id, action.label())?;
        if expected_directory_revision.is_empty()
            || expected_directory_revision != preview.directory_revision
        {
            return Err(WorkspaceError::DirectoryChanged);
        }
        if preview.conflict.is_some() {
            return Err(WorkspaceError::NameConflict {
                name: preview.name,
                source_label: preview.destination_source.unwrap_or_default(),
            });
        }

        let skill = self.find_skill(id)?;
        let target_source = action
            .destination_source(skill.source)?
            .ok_or(WorkspaceError::LifecycleNotAllowed)?;
        let source_directory = validated_skill_directory(&skill)?;
        if directory_revision(&source_directory)? != expected_directory_revision {
            return Err(WorkspaceError::DirectoryChanged);
        }
        let target_root = self.managed_root_for_lifecycle(target_source)?;
        let destination = target_root.join(&skill.summary.directory_name);
        if !destination.starts_with(&target_root) {
            return Err(WorkspaceError::UnsafePath);
        }
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(WorkspaceError::NameConflict {
                name: skill.summary.name,
                source_label: target_source.label().into(),
            });
        }

        fs::rename(&source_directory, &destination)?;
        let moved = self
            .read_skill(&destination, target_source, &target_root)?
            .ok_or(WorkspaceError::UnsafePath)?;
        Ok(LifecycleResult {
            ok: true,
            id: moved.summary.id,
            source: target_source.label().into(),
            destination: destination.display().to_string(),
            directory_revision: directory_revision(&destination)?,
            restart_recommended: true,
        })
    }

    pub fn delete_archived_skill(
        &self,
        id: &str,
        expected_directory_revision: &str,
        confirmation_name: &str,
    ) -> Result<DeleteSkillResult, WorkspaceError> {
        let preview = self.preview_skill_lifecycle(id, "delete")?;
        if confirmation_name != preview.name {
            return Err(WorkspaceError::DeleteConfirmationMismatch);
        }
        if expected_directory_revision.is_empty()
            || expected_directory_revision != preview.directory_revision
        {
            return Err(WorkspaceError::DirectoryChanged);
        }
        let skill = self.find_skill(id)?;
        if skill.source != Source::Archive {
            return Err(WorkspaceError::LifecycleNotAllowed);
        }
        let directory = validated_skill_directory(&skill)?;
        if directory_revision(&directory)? != expected_directory_revision {
            return Err(WorkspaceError::DirectoryChanged);
        }
        fs::remove_dir_all(directory)?;
        Ok(DeleteSkillResult {
            ok: true,
            deleted_name: preview.name,
            restart_recommended: true,
        })
    }

    fn root_for_source(&self, source: Source) -> PathBuf {
        let roots = self.roots();
        match source {
            Source::Personal => roots.personal,
            Source::Disabled => roots.disabled,
            Source::System => roots.system,
            Source::Plugin => roots.plugin,
            Source::Archive => roots.archive,
        }
    }

    fn managed_root_for_lifecycle(&self, source: Source) -> Result<PathBuf, WorkspaceError> {
        if !matches!(
            source,
            Source::Personal | Source::Disabled | Source::Archive
        ) {
            return Err(WorkspaceError::LifecycleNotAllowed);
        }
        let requested = self.root_for_source(source);
        fs::create_dir_all(&requested)?;
        let metadata = fs::symlink_metadata(&requested)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(WorkspaceError::UnsafePath);
        }
        Ok(fs::canonicalize(requested)?)
    }
}

fn validated_skill_directory(skill: &InternalSkill) -> Result<PathBuf, WorkspaceError> {
    let root_metadata = fs::symlink_metadata(&skill.root)?;
    let directory_metadata = fs::symlink_metadata(&skill.directory)?;
    if root_metadata.file_type().is_symlink()
        || directory_metadata.file_type().is_symlink()
        || !root_metadata.is_dir()
        || !directory_metadata.is_dir()
    {
        return Err(WorkspaceError::UnsafePath);
    }
    let root = fs::canonicalize(&skill.root)?;
    let directory = fs::canonicalize(&skill.directory)?;
    if directory.parent() != Some(root.as_path()) {
        return Err(WorkspaceError::UnsafePath);
    }
    directory_revision(&directory)?;
    Ok(directory)
}

fn directory_revision(root: &Path) -> Result<String, WorkspaceError> {
    let mut records = Vec::new();
    collect_records(root, root, 0, &mut records)?;
    records.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digest = Sha256::new();
    for (path, kind, content_hash) in records {
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update([kind]);
        digest.update([0]);
        digest.update(content_hash.as_bytes());
        digest.update([0]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn collect_records(
    root: &Path,
    current: &Path,
    depth: usize,
    records: &mut Vec<(String, u8, String)>,
) -> Result<(), WorkspaceError> {
    if depth > MAX_SCAN_DEPTH {
        return Err(WorkspaceError::UnsafePath);
    }
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| WorkspaceError::UnsafePath)?
            .to_string_lossy()
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::UnsafePath);
        }
        if metadata.is_dir() {
            records.push((relative, b'd', String::new()));
            collect_records(root, &path, depth + 1, records)?;
        } else if metadata.is_file() {
            records.push((relative, b'f', hash_file(&path)?));
        } else {
            return Err(WorkspaceError::UnsafePath);
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, WorkspaceError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn markdown(name: &str) -> String {
        format!(
            "---\nname: {name}\ndescription: >-\n  Use when the user asks for lifecycle testing.\n---\n\n# {name}\n\n1. Read the request.\n2. Return a detailed result.\n"
        )
    }

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let workspace = Workspace {
            codex_home: directory.path().to_path_buf(),
        };
        (directory, workspace)
    }

    fn write_skill(root: &Path, relative: &str, name: &str) {
        let directory = root.join(relative);
        fs::create_dir_all(directory.join("scripts")).expect("skill directory");
        fs::write(directory.join("SKILL.md"), markdown(name)).expect("skill document");
        fs::write(directory.join("scripts/helper.sh"), "echo helper\n").expect("supporting file");
    }

    fn skill_id(workspace: &Workspace, source: &str, name: &str) -> String {
        workspace
            .list_skills()
            .expect("catalog")
            .skills
            .into_iter()
            .find(|skill| skill.source == source && skill.name == name)
            .expect("skill")
            .id
    }

    #[test]
    fn all_valid_transitions_preserve_supporting_files_then_delete_from_archive() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/lifecycle", "lifecycle");

        let personal_id = skill_id(&workspace, "personal", "lifecycle");
        let disable = workspace
            .preview_skill_lifecycle(&personal_id, "disable")
            .expect("disable preview");
        assert!(disable.can_apply);
        assert!(!directory.path().join("skills-disabled/lifecycle").exists());
        let disabled = workspace
            .apply_skill_lifecycle(&personal_id, "disable", &disable.directory_revision)
            .expect("disable");
        assert_eq!(disabled.source, "disabled");
        assert!(directory
            .path()
            .join("skills-disabled/lifecycle/scripts/helper.sh")
            .exists());

        let enable = workspace
            .preview_skill_lifecycle(&disabled.id, "enable")
            .expect("enable preview");
        let personal = workspace
            .apply_skill_lifecycle(&disabled.id, "enable", &enable.directory_revision)
            .expect("enable");
        assert_eq!(personal.source, "personal");

        let archive = workspace
            .preview_skill_lifecycle(&personal.id, "archive")
            .expect("archive preview");
        let archived = workspace
            .apply_skill_lifecycle(&personal.id, "archive", &archive.directory_revision)
            .expect("archive");
        assert_eq!(archived.source, "archive");

        let restore = workspace
            .preview_skill_lifecycle(&archived.id, "restore")
            .expect("restore preview");
        let restored = workspace
            .apply_skill_lifecycle(&archived.id, "restore", &restore.directory_revision)
            .expect("restore");
        assert_eq!(restored.source, "personal");

        let archive_again = workspace
            .preview_skill_lifecycle(&restored.id, "archive")
            .expect("second archive preview");
        let archived_again = workspace
            .apply_skill_lifecycle(&restored.id, "archive", &archive_again.directory_revision)
            .expect("second archive");
        let delete = workspace
            .preview_skill_lifecycle(&archived_again.id, "delete")
            .expect("delete preview");
        assert!(matches!(
            workspace.delete_archived_skill(
                &archived_again.id,
                &delete.directory_revision,
                "wrong-name"
            ),
            Err(WorkspaceError::DeleteConfirmationMismatch)
        ));
        assert!(directory.path().join("skill-archive/lifecycle").exists());
        workspace
            .delete_archived_skill(&archived_again.id, &delete.directory_revision, "lifecycle")
            .expect("permanent delete");
        assert!(!directory.path().join("skill-archive/lifecycle").exists());
    }

    #[test]
    fn destination_conflict_never_overwrites_either_skill() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/source-skill", "source-skill");
        write_skill(
            directory.path(),
            "skills-disabled/source-skill",
            "existing-disabled",
        );
        let id = skill_id(&workspace, "personal", "source-skill");
        let preview = workspace
            .preview_skill_lifecycle(&id, "disable")
            .expect("conflict preview");
        assert!(!preview.can_apply);
        assert!(preview.conflict.is_some());
        assert!(matches!(
            workspace.apply_skill_lifecycle(&id, "disable", &preview.directory_revision),
            Err(WorkspaceError::NameConflict { .. })
        ));
        assert!(directory
            .path()
            .join("skills/source-skill/SKILL.md")
            .exists());
        assert!(directory
            .path()
            .join("skills-disabled/source-skill/SKILL.md")
            .exists());
    }

    #[test]
    fn stale_directory_revision_blocks_transition() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/stale", "stale");
        let id = skill_id(&workspace, "personal", "stale");
        let preview = workspace
            .preview_skill_lifecycle(&id, "archive")
            .expect("archive preview");
        fs::write(
            directory.path().join("skills/stale/scripts/helper.sh"),
            "echo changed\n",
        )
        .expect("concurrent change");
        assert!(matches!(
            workspace.apply_skill_lifecycle(&id, "archive", &preview.directory_revision),
            Err(WorkspaceError::DirectoryChanged)
        ));
        assert!(directory.path().join("skills/stale").exists());
        assert!(!directory.path().join("skill-archive/stale").exists());
    }

    #[test]
    fn invalid_sources_and_actions_are_rejected() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/personal", "personal");
        write_skill(directory.path(), "skills/.system/managed", "managed");
        let personal = skill_id(&workspace, "personal", "personal");
        let managed = skill_id(&workspace, "system", "managed");
        assert!(matches!(
            workspace.preview_skill_lifecycle(&personal, "enable"),
            Err(WorkspaceError::LifecycleNotAllowed)
        ));
        assert!(matches!(
            workspace.preview_skill_lifecycle(&managed, "archive"),
            Err(WorkspaceError::LifecycleNotAllowed)
        ));
        assert!(matches!(
            workspace.preview_skill_lifecycle(&personal, "unknown"),
            Err(WorkspaceError::InvalidLifecycleAction)
        ));
    }

    #[test]
    fn failed_target_root_creation_leaves_source_intact() {
        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/rename-fails", "rename-fails");
        let id = skill_id(&workspace, "personal", "rename-fails");
        let preview = workspace
            .preview_skill_lifecycle(&id, "disable")
            .expect("disable preview");
        fs::write(directory.path().join("skills-disabled"), "not a directory")
            .expect("blocking file");
        assert!(matches!(
            workspace.apply_skill_lifecycle(&id, "disable", &preview.directory_revision),
            Err(WorkspaceError::Io(_))
        ));
        assert!(directory.path().join("skills/rename-fails").exists());
    }

    #[cfg(unix)]
    #[test]
    fn nested_symlinks_are_rejected_before_preview() {
        use std::os::unix::fs::symlink;

        let (directory, workspace) = workspace();
        write_skill(directory.path(), "skills/linked", "linked");
        let external = directory.path().join("outside.txt");
        fs::write(&external, "outside").expect("external file");
        symlink(
            &external,
            directory.path().join("skills/linked/scripts/outside-link"),
        )
        .expect("symlink");
        let id = skill_id(&workspace, "personal", "linked");
        assert!(matches!(
            workspace.preview_skill_lifecycle(&id, "archive"),
            Err(WorkspaceError::UnsafePath)
        ));
        assert!(external.exists());
    }
}
