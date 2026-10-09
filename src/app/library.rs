//! My Modules: groups saved to a folder of your own, to add to any patch.
//!
//! Each saved group is a small patch file holding just that group, named
//! after it, in `Documents/Modular/My Modules`. Adding one is a paste of
//! that patch, so it comes in whole: its modules, cables, jacks and
//! pinned knobs. Saving a group under a name that's already there updates
//! that module.

use std::path::{Path, PathBuf};

use crate::persistence::{load_from_file, save_to_file, GroupData, Patch, PatchError};

/// Where saved groups live.
pub fn folder() -> PathBuf {
    dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Documents")))
        .unwrap_or_else(std::env::temp_dir)
        .join("Modular")
        .join("My Modules")
}

/// A group saved to My Modules.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedModule {
    pub name: String,
    pub path: PathBuf,
    /// What it is, in a line: its jacks and how many modules are inside.
    pub summary: String,
}

impl SavedModule {
    fn of(patch: &Patch, path: &Path) -> Option<Self> {
        let [group] = patch.groups.as_slice() else { return None };
        if !patch.nodes.is_empty() {
            return None;
        }
        Some(Self { name: group.name.clone(), path: path.to_path_buf(), summary: summary(group) })
    }

    /// The saved group, as a patch to paste.
    pub fn load(&self) -> Result<Patch, PatchError> {
        load_from_file(&self.path)
    }
}

/// "In, Gate → Out · 4 modules"
fn summary(group: &GroupData) -> String {
    fn count(group: &GroupData) -> usize {
        group.nodes.len() + group.groups.iter().map(count).sum::<usize>()
    }
    let names = |jacks: &[crate::persistence::JackData]| {
        if jacks.is_empty() {
            "nothing".to_string()
        } else {
            jacks.iter().map(|j| j.name.as_str()).collect::<Vec<_>>().join(", ")
        }
    };
    let modules = match count(group) {
        1 => "1 module".to_string(),
        n => format!("{n} modules"),
    };
    format!("{} → {} · {}", names(&group.inputs), names(&group.outputs), modules)
}

/// Every module saved in `folder`, by name. Files that aren't a saved group
/// are passed over.
pub fn list_in(folder: &Path) -> Vec<SavedModule> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut modules: Vec<SavedModule> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .filter_map(|path| SavedModule::of(&load_from_file(&path).ok()?, &path))
        .collect();
    modules.sort_by_key(|m| m.name.to_lowercase());
    modules
}

/// Every module in My Modules.
pub fn list() -> Vec<SavedModule> {
    list_in(&folder())
}

/// A file name for a module's name: the characters a file name can't have
/// become spaces.
pub fn file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.');
    format!("{}.json", if cleaned.is_empty() { "Group" } else { cleaned })
}

/// Saves a patch holding one group to `folder` under the group's name.
/// Returns where it went, and whether it replaced a module of that name.
pub fn save_in(folder: &Path, patch: &Patch) -> Result<(PathBuf, bool), PatchError> {
    std::fs::create_dir_all(folder)?;
    let path = folder.join(file_name(&patch.name));
    let replaced = path.exists();
    save_to_file(patch, &path)?;
    Ok((path, replaced))
}

/// Saves a patch holding one group to My Modules.
pub fn save(patch: &Patch) -> Result<(PathBuf, bool), PatchError> {
    save_in(&folder(), patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{ConnectionData, JackData, NodeData};

    fn voice() -> Patch {
        let mut patch = Patch::new("Voice: Lead");
        patch.groups.push(GroupData {
            id: 10,
            name: "Voice: Lead".into(),
            position: (0.0, 0.0),
            inputs: vec![JackData { name: "Gate".into(), signal: "Gate".into() }],
            outputs: vec![JackData { name: "Out".into(), signal: "Audio".into() }],
            inputs_position: (-200.0, 0.0),
            outputs_position: (400.0, 0.0),
            nodes: vec![NodeData::new(1, "mod.adsr", (0.0, 0.0)), NodeData::new(2, "util.vca", (200.0, 0.0))],
            connections: vec![
                ConnectionData::new(10, "Gate", 1, "Gate"),
                ConnectionData::new(1, "Out", 2, "CV"),
                ConnectionData::new(2, "Out", 10, "Out"),
            ],
            groups: Vec::new(),
        });
        patch.version = patch.required_version();
        patch
    }

    #[test]
    fn saved_groups_list_by_name_with_a_summary() {
        let folder = std::env::temp_dir().join(format!("modular-my-modules-{}", std::process::id()));
        let (path, replaced) = save_in(&folder, &voice()).unwrap();
        assert!(!replaced);
        assert_eq!(path.file_name().unwrap(), "Voice  Lead.json");
        // A file that isn't a saved group is passed over
        std::fs::write(folder.join("notes.json"), "{}").unwrap();

        let listed = list_in(&folder);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Voice: Lead");
        assert_eq!(listed[0].summary, "Gate → Out · 2 modules");
        assert_eq!(listed[0].load().unwrap().groups.len(), 1);

        // Saving under the same name updates it
        let (_, replaced) = save_in(&folder, &voice()).unwrap();
        assert!(replaced);
        assert_eq!(list_in(&folder).len(), 1);
        std::fs::remove_dir_all(&folder).ok();
    }

    #[test]
    fn names_make_safe_file_names() {
        assert_eq!(file_name("Bass / Sub?"), "Bass   Sub.json");
        assert_eq!(file_name("..."), "Group.json");
        assert_eq!(file_name("Pad"), "Pad.json");
    }
}
