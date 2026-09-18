use crate::{files, git};
use anyhow::{Context, Result, bail};
use std::path::Path;
use url::Url;

#[derive(Debug, PartialEq)]
enum Kind {
    Local,
    Url,
    Scp,
}

fn kind(value: &str) -> Result<Kind> {
    if value.trim().is_empty() || value.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
        bail!("A repository reference must be a non-empty URL or path.");
    }
    if value.split_once("::").is_some_and(|(prefix, _)| {
        prefix
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && prefix
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+.-".contains(&c))
    }) {
        bail!("Git remote helpers are not allowed.");
    }
    if value.contains("://") {
        let url = Url::parse(value).context("Invalid repository URL")?;
        if !["https", "ssh", "file"].contains(&url.scheme()) {
            bail!("Use HTTPS, SSH or a local Git path.");
        }
        if url.password().is_some()
            || (url.scheme() == "https" && !url.username().is_empty())
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("Do not put credentials, query parameters or fragments in repository URLs.");
        }
        return Ok(Kind::Url);
    }
    if let Some((left, right)) = value.split_once(':')
        && left.contains('@')
        && !left.chars().any(|c| c.is_whitespace() || c == '/')
        && !right.is_empty()
    {
        return Ok(Kind::Scp);
    }
    Ok(Kind::Local)
}

pub fn validate(value: &str) -> Result<()> {
    kind(value).map(|_| ())
}

pub fn normalize(value: &str, cwd: &Path) -> Result<String> {
    match kind(value)? {
        Kind::Url if value.starts_with("file://") => {
            let path = Url::parse(value)?
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Invalid file URL"))?;
            Ok(files::absolute(path)?.to_string_lossy().into_owned())
        }
        Kind::Local => Ok(files::absolute(cwd.join(value))?
            .to_string_lossy()
            .into_owned()),
        _ => Ok(value.to_owned()),
    }
}

fn posix_normalize(value: &str) -> String {
    let mut parts = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    format!(
        "{}{}",
        if value.starts_with('/') { "/" } else { "" },
        parts.join("/")
    )
}

pub fn resolve(reference: &str, source: &str) -> Result<String> {
    let reference_kind = kind(reference)?;
    let source = normalize(source, &std::env::current_dir()?)?;
    let source_kind = kind(&source)?;
    if !reference.starts_with("./") && !reference.starts_with("../") {
        let is_local = reference_kind == Kind::Local || reference.starts_with("file://");
        if source_kind != Kind::Local && is_local {
            bail!("A remote spec cannot reference an absolute local repository path.");
        }
        return normalize(reference, &std::env::current_dir()?);
    }
    match source_kind {
        Kind::Url => Ok(Url::parse(&format!("{}/", source.trim_end_matches('/')))?
            .join(reference)?
            .to_string()),
        Kind::Scp => {
            let (host, remote_path) = source.split_once(':').context("Invalid SSH source")?;
            Ok(format!(
                "{host}:{}",
                posix_normalize(&format!("{remote_path}/{reference}"))
            ))
        }
        Kind::Local => normalize(reference, Path::new(&source)),
    }
}

pub fn origin(path: &Path) -> Result<String> {
    match git::run(path, ["remote", "get-url", "origin"]) {
        Ok(value) => normalize(&value, path),
        Err(_) => Ok(path.to_string_lossy().into_owned()),
    }
}

pub fn namespace(value: &str, cwd: &Path) -> Result<String> {
    if files::portable_name(value) {
        Ok(format!("git@github.com:{value}"))
    } else {
        Ok(normalize(value, cwd)?.trim_end_matches('/').to_owned())
    }
}

pub fn spec_source(value: &str, namespace_value: Option<&str>, cwd: &Path) -> Result<String> {
    if files::portable_name(value) {
        let base = namespace_value.context("Configure a repository namespace first: cspec config namespace <GitHub-owner-or-namespace-URL>")?;
        let base = namespace(base, cwd)?;
        Ok(if kind(&base)? == Kind::Local {
            Path::new(&base).join(value).to_string_lossy().into_owned()
        } else {
            format!("{base}/{value}")
        })
    } else {
        normalize(value, cwd)
    }
}

pub fn directory_name(source: &str) -> Result<String> {
    let value = match kind(source)? {
        Kind::Url => Url::parse(source)?.path().to_owned(),
        Kind::Scp => source
            .split_once(':')
            .context("Invalid SSH source")?
            .1
            .to_owned(),
        Kind::Local => source.to_owned(),
    };
    let trimmed = value.trim_end_matches(['/', '\\']);
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    let name = name.strip_suffix(".git").unwrap_or(name);
    if !files::portable_name(name) {
        bail!("Repository directory name is not portable: {name}");
    }
    Ok(name.to_owned())
}

pub fn identity(value: &str) -> Result<String> {
    let value = normalize(value, &std::env::current_dir()?)?;
    let remote = match kind(&value)? {
        Kind::Url => {
            let url = Url::parse(&value)?;
            let port = url
                .port()
                .filter(|p| ![22, 443].contains(p))
                .map(|p| format!(":{p}"))
                .unwrap_or_default();
            format!(
                "{}{port}{}",
                url.host_str()
                    .context("Repository URL has no host")?
                    .to_lowercase(),
                url.path()
            )
        }
        Kind::Scp => {
            let (host, path) = value.split_once(':').context("Invalid SSH source")?;
            format!(
                "{}/{}",
                host.rsplit('@').next().unwrap_or(host).to_lowercase(),
                path
            )
        }
        Kind::Local => {
            return Ok(if cfg!(windows) {
                value.to_lowercase()
            } else {
                value
            });
        }
    };
    let remote = remote.trim_end_matches('/');
    Ok(remote.strip_suffix(".git").unwrap_or(remote).to_owned())
}
