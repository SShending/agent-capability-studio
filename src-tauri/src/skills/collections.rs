use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAX_COLLECTIONS: usize = 100;
const MAX_NAME_CHARS: usize = 60;
const MAX_MEMBERS: usize = 10_000;

#[derive(Debug, Error)]
pub enum CollectionsError {
    #[error("The Collection name must contain 1 to 60 visible characters.")]
    InvalidName,
    #[error("That Collection was not found.")]
    NotFound,
    #[error("A Collection with that name already exists.")]
    DuplicateName,
    #[error("The Collections file is invalid or exceeds supported limits.")]
    InvalidStore,
    #[error("Unable to access Collections: {0}")]
    Io(#[from] std::io::Error),
}

impl CollectionsError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidName => "INVALID_COLLECTION_NAME",
            Self::NotFound => "COLLECTION_NOT_FOUND",
            Self::DuplicateName => "DUPLICATE_COLLECTION_NAME",
            Self::InvalidStore => "INVALID_COLLECTION_STORE",
            Self::Io(_) => "COLLECTION_IO_ERROR",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub member_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSnapshot {
    pub collections: Vec<Collection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredCollection {
    id: String,
    name: String,
    member_ids: BTreeSet<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Store {
    version: u8,
    collections: Vec<StoredCollection>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: 1,
            collections: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct CollectionManager {
    path: PathBuf,
    mutation: Arc<Mutex<()>>,
}

impl CollectionManager {
    pub fn new(settings_directory: PathBuf) -> Self {
        Self {
            path: settings_directory.join("collections.json"),
            mutation: Arc::new(Mutex::new(())),
        }
    }

    pub fn list(&self, known_skill_ids: &[String]) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let known = known_skill_ids.iter().collect::<BTreeSet<_>>();
        let mut store = self.read()?;
        let mut changed = false;
        for collection in &mut store.collections {
            let before = collection.member_ids.len();
            collection.member_ids.retain(|id| known.contains(id));
            changed |= collection.member_ids.len() != before;
        }
        if changed {
            self.write(&store)?;
        }
        snapshot(store)
    }

    pub fn create(&self, name: &str) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let name = validated_name(name)?;
        let mut store = self.read()?;
        if store.collections.len() >= MAX_COLLECTIONS {
            return Err(CollectionsError::InvalidStore);
        }
        ensure_unique_name(&store, &name, None)?;
        let next = store
            .collections
            .iter()
            .filter_map(|item| item.id.strip_prefix("collection-")?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        store.collections.push(StoredCollection {
            id: format!("collection-{next}"),
            name,
            member_ids: BTreeSet::new(),
        });
        self.write(&store)?;
        snapshot(store)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let name = validated_name(name)?;
        let mut store = self.read()?;
        ensure_unique_name(&store, &name, Some(id))?;
        let collection = store
            .collections
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or(CollectionsError::NotFound)?;
        collection.name = name;
        self.write(&store)?;
        snapshot(store)
    }

    pub fn delete(&self, id: &str) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        let before = store.collections.len();
        store.collections.retain(|item| item.id != id);
        if store.collections.len() == before {
            return Err(CollectionsError::NotFound);
        }
        self.write(&store)?;
        snapshot(store)
    }

    pub fn set_skill_memberships(
        &self,
        skill_id: &str,
        collection_ids: &[String],
    ) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if skill_id.is_empty() || skill_id.len() > 4096 {
            return Err(CollectionsError::InvalidStore);
        }
        let selected = collection_ids.iter().cloned().collect::<BTreeSet<_>>();
        let mut store = self.read()?;
        if selected.len() != collection_ids.len()
            || selected
                .iter()
                .any(|id| !store.collections.iter().any(|item| &item.id == id))
        {
            return Err(CollectionsError::NotFound);
        }
        for collection in &mut store.collections {
            if selected.contains(&collection.id) {
                if collection.member_ids.len() >= MAX_MEMBERS
                    && !collection.member_ids.contains(skill_id)
                {
                    return Err(CollectionsError::InvalidStore);
                }
                collection.member_ids.insert(skill_id.to_string());
            } else {
                collection.member_ids.remove(skill_id);
            }
        }
        self.write(&store)?;
        snapshot(store)
    }

    pub fn replace_member(
        &self,
        previous_id: &str,
        next_id: Option<&str>,
    ) -> Result<CollectionSnapshot, CollectionsError> {
        let _guard = self
            .mutation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut store = self.read()?;
        let mut changed = false;
        for collection in &mut store.collections {
            if collection.member_ids.remove(previous_id) {
                changed = true;
                if let Some(next_id) = next_id {
                    if next_id.is_empty() || next_id.len() > 4096 {
                        return Err(CollectionsError::InvalidStore);
                    }
                    collection.member_ids.insert(next_id.to_string());
                }
            }
        }
        if changed {
            self.write(&store)?;
        }
        snapshot(store)
    }

    pub(crate) fn memberships_for(&self, skill_id: &str) -> Result<Vec<String>, CollectionsError> {
        if skill_id.is_empty() || skill_id.len() > 4096 {
            return Err(CollectionsError::InvalidStore);
        }
        Ok(self
            .read()?
            .collections
            .into_iter()
            .filter(|collection| collection.member_ids.contains(skill_id))
            .map(|collection| collection.id)
            .collect())
    }

    fn read(&self) -> Result<Store, CollectionsError> {
        let content = match fs::read_to_string(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Store::default())
            }
            Err(error) => return Err(error.into()),
        };
        if content.len() > 4 * 1024 * 1024 {
            return Err(CollectionsError::InvalidStore);
        }
        let store: Store =
            serde_json::from_str(&content).map_err(|_| CollectionsError::InvalidStore)?;
        validate_store(&store)?;
        Ok(store)
    }

    fn write(&self, store: &Store) -> Result<(), CollectionsError> {
        validate_store(store)?;
        let parent = self.path.parent().ok_or(CollectionsError::InvalidStore)?;
        fs::create_dir_all(parent)?;
        let bytes = serde_json::to_vec_pretty(store).map_err(|_| CollectionsError::InvalidStore)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file_mut().sync_all()?;
        temporary
            .persist(&self.path)
            .map_err(|error| CollectionsError::Io(error.error))?;
        set_private_permissions(parent, &self.path)?;
        Ok(())
    }
}

fn validated_name(value: &str) -> Result<String, CollectionsError> {
    let name = value.trim();
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err(CollectionsError::InvalidName);
    }
    Ok(name.to_string())
}

fn ensure_unique_name(
    store: &Store,
    name: &str,
    ignored_id: Option<&str>,
) -> Result<(), CollectionsError> {
    if store
        .collections
        .iter()
        .any(|item| Some(item.id.as_str()) != ignored_id && item.name.eq_ignore_ascii_case(name))
    {
        Err(CollectionsError::DuplicateName)
    } else {
        Ok(())
    }
}

fn validate_store(store: &Store) -> Result<(), CollectionsError> {
    if store.version != 1 || store.collections.len() > MAX_COLLECTIONS {
        return Err(CollectionsError::InvalidStore);
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for collection in &store.collections {
        validated_name(&collection.name)?;
        if collection.id.is_empty()
            || !ids.insert(collection.id.clone())
            || !names.insert(collection.name.to_lowercase())
            || collection.member_ids.len() > MAX_MEMBERS
        {
            return Err(CollectionsError::InvalidStore);
        }
    }
    Ok(())
}

fn snapshot(mut store: Store) -> Result<CollectionSnapshot, CollectionsError> {
    validate_store(&store)?;
    store
        .collections
        .sort_by_key(|item| item.name.to_lowercase());
    Ok(CollectionSnapshot {
        collections: store
            .collections
            .into_iter()
            .map(|item| Collection {
                id: item.id,
                name: item.name,
                member_ids: item.member_ids.into_iter().collect(),
            })
            .collect(),
    })
}

#[cfg(unix)]
fn set_private_permissions(directory: &Path, file: &Path) -> Result<(), CollectionsError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(file, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_directory: &Path, _file: &Path) -> Result<(), CollectionsError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_many_to_many_membership_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CollectionManager::new(directory.path().into());
        let created = manager.create("Research").unwrap();
        let id = &created.collections[0].id;
        manager
            .set_skill_memberships("skill-a", std::slice::from_ref(id))
            .unwrap();
        manager
            .set_skill_memberships("skill-b", std::slice::from_ref(id))
            .unwrap();
        let restarted = CollectionManager::new(directory.path().into())
            .list(&["skill-a".into(), "skill-b".into()])
            .unwrap();
        assert_eq!(
            restarted.collections[0].member_ids,
            vec!["skill-a", "skill-b"]
        );
    }

    #[test]
    fn deleting_collection_does_not_touch_skills() {
        let directory = tempfile::tempdir().unwrap();
        let skill = directory.path().join("skill/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "content").unwrap();
        let manager = CollectionManager::new(directory.path().join("settings"));
        let id = manager.create("Engineering").unwrap().collections[0]
            .id
            .clone();
        manager
            .set_skill_memberships("skill-a", std::slice::from_ref(&id))
            .unwrap();
        assert!(manager.delete(&id).unwrap().collections.is_empty());
        assert_eq!(fs::read_to_string(skill).unwrap(), "content");
    }

    #[test]
    fn rejects_duplicate_and_invalid_names() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CollectionManager::new(directory.path().into());
        manager.create("Research").unwrap();
        assert!(matches!(
            manager.create("research"),
            Err(CollectionsError::DuplicateName)
        ));
        assert!(matches!(
            manager.create("\n"),
            Err(CollectionsError::InvalidName)
        ));
    }

    #[test]
    fn replaces_or_removes_members_across_every_collection() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CollectionManager::new(directory.path().into());
        let first = manager.create("One").unwrap().collections[0].id.clone();
        let second = manager.create("Two").unwrap().collections[1].id.clone();
        manager
            .set_skill_memberships("old", &[first.clone(), second.clone()])
            .unwrap();
        let moved = manager.replace_member("old", Some("new")).unwrap();
        assert!(moved
            .collections
            .iter()
            .all(|item| item.member_ids == vec!["new"]));
        let removed = manager.replace_member("new", None).unwrap();
        assert!(removed
            .collections
            .iter()
            .all(|item| item.member_ids.is_empty()));
    }

    #[test]
    fn replaces_one_skills_memberships_in_one_atomic_store_write() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CollectionManager::new(directory.path().into());
        let first = manager.create("One").unwrap().collections[0].id.clone();
        let second = manager.create("Two").unwrap().collections[1].id.clone();
        manager
            .set_skill_memberships("skill", std::slice::from_ref(&first))
            .unwrap();
        let updated = manager
            .set_skill_memberships("skill", std::slice::from_ref(&second))
            .unwrap();
        assert!(!updated
            .collections
            .iter()
            .find(|item| item.id == first)
            .unwrap()
            .member_ids
            .contains(&"skill".into()));
        assert!(updated
            .collections
            .iter()
            .find(|item| item.id == second)
            .unwrap()
            .member_ids
            .contains(&"skill".into()));
    }

    #[test]
    fn list_prunes_members_that_are_missing_from_the_current_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let manager = CollectionManager::new(directory.path().into());
        let id = manager.create("Research").unwrap().collections[0]
            .id
            .clone();
        manager
            .set_skill_memberships("present-skill", std::slice::from_ref(&id))
            .unwrap();
        manager
            .set_skill_memberships("missing-skill", std::slice::from_ref(&id))
            .unwrap();

        let pruned = manager.list(&["present-skill".into()]).unwrap();
        assert_eq!(pruned.collections[0].member_ids, vec!["present-skill"]);

        let restarted = CollectionManager::new(directory.path().into())
            .list(&["present-skill".into(), "missing-skill".into()])
            .unwrap();
        assert_eq!(restarted.collections[0].member_ids, vec!["present-skill"]);
    }
}
