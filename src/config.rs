use crate::{files, git, repository};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub fn path() -> Result<PathBuf> {
    let root = match std::env::var_os("CRETSPEC_HOME").filter(|s| !s.is_empty()) {
        Some(root) => PathBuf::from(root),
        None => {
            let home = if cfg!(windows) {
                std::env::var_os("USERPROFILE")
            } else {
                std::env::var_os("HOME")
            };
            PathBuf::from(
                home.context("Home directory is unavailable. Set CRETSPEC_HOME explicitly")?,
            )
            .join(".cretspec")
        }
    };
    Ok(files::absolute(root)?.join("config.json"))
}

fn read() -> Result<Option<Value>> {
    let path = path()?;
    if !files::exists(&path)? {
        return Ok(None);
    }
    let value: Value = files::read_json(&path)?;
    let current = value["schemaVersion"] == 2 && value["repositoryNamespace"].is_string();
    let legacy = value["schemaVersion"] == 1 && value["guidelinesRoot"].is_string();
    if !value.is_object() || (!current && !legacy) {
        bail!("Invalid CretSpec configuration: {}", path.display());
    }
    Ok(Some(value))
}

pub fn get_namespace() -> Result<Option<String>> {
    let Some(value) = read()? else {
        return Ok(None);
    };
    if let Some(namespace) = value["repositoryNamespace"]
        .as_str()
        .filter(|_| value["schemaVersion"] == 2)
    {
        return Ok(Some(repository::namespace(
            namespace,
            &std::env::current_dir()?,
        )?));
    }
    let directory = files::absolute(
        value["guidelinesRoot"]
            .as_str()
            .context("Invalid legacy guidelines path")?,
    )?;
    let result = git::root(&directory)
        .and_then(|root| repository::resolve("../", &repository::origin(&root)?));
    Ok(Some(result.context("The old configuration points to a guidelines clone that has moved. Set the repository namespace with: cspec config namespace <GitHub-owner-or-namespace-URL>")?))
}

pub fn set_namespace(value: &str) -> Result<String> {
    let namespace = repository::namespace(value, &std::env::current_dir()?)?;
    let mut settings = read()?.unwrap_or_else(|| json!({}));
    let map = settings
        .as_object_mut()
        .context("Invalid CretSpec configuration")?;
    map.remove("guidelinesRoot");
    map.insert("schemaVersion".into(), json!(2));
    map.insert("repositoryNamespace".into(), json!(namespace));
    let path = path()?;
    fs::create_dir_all(path.parent().context("Configuration has no parent")?)?;
    files::write_json(&path, &settings)?;
    Ok(namespace)
}
