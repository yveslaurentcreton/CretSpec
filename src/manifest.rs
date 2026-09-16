use crate::{files, repository};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    pub code: Source,
    pub guidelines: Guidelines,
    pub profile: String,
    #[serde(default = "default_agents")]
    pub agents: Vec<Agent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
    Copilot,
    Cursor,
}

pub fn default_agents() -> Vec<Agent> {
    vec![Agent::Codex, Agent::Claude, Agent::Copilot, Agent::Cursor]
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guidelines {
    pub repository: String,
    #[serde(rename = "ref")]
    pub reference: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lock {
    pub schema_version: u32,
    #[serde(rename = "ref")]
    pub reference: String,
    pub commit: String,
}

pub fn valid_ref(reference: &str) -> bool {
    reference
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && reference
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

pub fn read(spec: &Path) -> Result<(Manifest, Lock)> {
    if files::exists(&spec.join(".local/definition-update.json"))? {
        bail!(
            "A guidelines update was interrupted. Run cspec guidelines recover with this project directory before continuing."
        );
    }
    let manifest: Manifest = files::read_json(&spec.join("project.json"))?;
    let lock: Lock = files::read_json(&spec.join("guidelines.lock.json"))?;
    validate(&manifest, &lock)?;
    files::directory(&spec.join("spec"))?;
    Ok((manifest, lock))
}

pub fn validate(manifest: &Manifest, lock: &Lock) -> Result<()> {
    for (index, agent) in manifest.agents.iter().enumerate() {
        if manifest.agents[..index].contains(agent) {
            bail!("Agent integrations must not contain duplicates.");
        }
    }
    if manifest.schema_version != 1 || lock.schema_version != 1 {
        bail!("Only schemaVersion 1 is supported.");
    }
    if !files::portable_name(&manifest.name) || !manifest.name.as_bytes()[0].is_ascii_alphabetic() {
        bail!("Invalid or non-portable project name.");
    }
    if !manifest
        .profile
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || !manifest
            .profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        bail!("Invalid profile.");
    }
    repository::validate(&manifest.code.repository)?;
    repository::validate(&manifest.guidelines.repository)?;
    if !valid_ref(&manifest.guidelines.reference) {
        bail!("Invalid guidelines ref.");
    }
    if lock.reference != manifest.guidelines.reference
        || ![40, 64].contains(&lock.commit.len())
        || !lock
            .commit
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        bail!("The lock must contain the same ref and a full exact commit ID.");
    }
    Ok(())
}
