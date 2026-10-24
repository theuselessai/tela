//! Downloaded apps are identified by their source, never by a manifest's name.
mod cache;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tls_tests;

use anyhow::{bail, ensure, Context, Result};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;
use std::time::Duration;

const MANIFEST_LIMIT: usize = 64 * 1024;
const BUNDLE_LIMIT: usize = 4 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
enum Permission {
    Network,
    Storage,
    Crypto,
    Credentials,
    Clipboard,
}

impl Permission {
    fn name(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Storage => "storage",
            Self::Crypto => "crypto",
            Self::Credentials => "credentials",
            Self::Clipboard => "clipboard",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    // Informational only: neither field is an identity or a consent key.
    #[serde(rename = "name")]
    _name: String,
    #[serde(rename = "version")]
    _version: String,
    entry: String,
    sha256: String,
    permissions: Vec<Permission>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Grant {
    source: String,
    sha256: String,
    permissions: Vec<Permission>,
}

pub(crate) struct App {
    pub(crate) namespace: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) bundle: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_url(url: &Url) -> Result<()> {
    ensure!(
        url.scheme() == "https" && url.host_str().is_some(),
        "remote URLs must use HTTPS"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "URL userinfo is forbidden"
    );
    ensure!(url.fragment().is_none(), "URL fragments are forbidden");
    Ok(())
}

fn source_url(input: &str) -> Result<Url> {
    // Reject even empty userinfo, which URL normalization would otherwise erase.
    ensure!(
        !input.chars().any(|c| c.is_control() || c.is_whitespace()),
        "whitespace in URL is forbidden"
    );
    ensure!(!input.contains('\\'), "URL backslashes are forbidden");
    let (_, rest) = input
        .split_once("://")
        .context("URL must have an explicit HTTPS authority")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    ensure!(!authority.is_empty(), "URL authority is required");
    ensure!(!authority.contains('@'), "URL userinfo is forbidden");
    let url = Url::parse(input).context("invalid manifest URL")?;
    validate_url(&url)?;
    Ok(url)
}

fn same_origin(source: &Url, target: &Url) -> Result<()> {
    validate_url(target)?;
    ensure!(
        source.origin() == target.origin(),
        "cross-origin download is forbidden"
    );
    Ok(())
}

fn bundle_url(source: &Url, entry: &str) -> Result<Url> {
    // A deliberately small canonical grammar: no URL escapes, queries, backslashes,
    // empty/dot segments, or schemes. This also rules out encoded traversal.
    ensure!(
        !entry.is_empty() && entry.len() <= 1024,
        "invalid bundle entry"
    );
    for segment in entry.split('/') {
        ensure!(
            !segment.is_empty() && segment != "." && segment != "..",
            "non-canonical bundle entry"
        );
        ensure!(
            segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "unsafe bundle entry"
        );
    }
    ensure!(
        entry.ends_with(".js"),
        "remote entry must be a JavaScript .js bundle"
    );
    let target = source.join(entry)?;
    same_origin(source, &target)?;
    Ok(target)
}

trait Download {
    async fn get(&self, url: &Url, limit: usize) -> Result<Vec<u8>>;
}

struct HttpDownload {
    client: Client,
    deadline: Duration,
}

impl HttpDownload {
    fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(DOWNLOAD_TIMEOUT)
                .build()?,
            deadline: DOWNLOAD_TIMEOUT,
        })
    }
}

fn redirect_target(source: &Url, current: &Url, location: &str) -> Result<Url> {
    // URL joining can normalize empty userinfo; reject it before parsing.
    ensure!(
        !location.contains('@')
            && !location
                .chars()
                .any(|c| c.is_control() || c.is_whitespace()),
        "unsafe redirect location"
    );
    let next = current
        .join(location)
        .context("invalid redirect location")?;
    same_origin(source, &next)?;
    Ok(next)
}

fn append_chunk(bytes: &mut Vec<u8>, chunk: &[u8], limit: usize) -> Result<()> {
    ensure!(
        chunk.len() <= limit.saturating_sub(bytes.len()),
        "download exceeds size limit"
    );
    bytes.extend_from_slice(chunk);
    Ok(())
}

impl Download for HttpDownload {
    async fn get(&self, source: &Url, limit: usize) -> Result<Vec<u8>> {
        validate_url(source)?;
        // One deadline includes all redirects and streaming, not one per chunk.
        tokio::time::timeout(self.deadline, async {
            let mut current = source.clone();
            for hop in 0..=5 {
                let mut response = self
                    .client
                    .get(current.clone())
                    .send()
                    .await
                    .map_err(|_| anyhow::anyhow!("HTTPS download failed"))?;
                if response.status().is_redirection() {
                    ensure!(hop < 5, "too many redirects");
                    let location = response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .context("redirect missing Location")?
                        .to_str()?;
                    current = redirect_target(source, &current, location)?;
                    continue;
                }
                ensure!(
                    response.status().is_success(),
                    "download returned HTTP {}",
                    response.status().as_u16()
                );
                ensure!(
                    response.content_length().is_none_or(|n| n <= limit as u64),
                    "download exceeds size limit"
                );
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| anyhow::anyhow!("HTTPS body read failed"))?
                {
                    append_chunk(&mut bytes, &chunk, limit)?;
                }
                return Ok(bytes);
            }
            bail!("too many redirects")
        })
        .await
        .context("download timed out")?
    }
}

trait Approval {
    fn approve(&mut self, grant: &Grant) -> Result<()>;
}

struct TerminalApproval;

fn ask(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    terminal: bool,
    grant: &Grant,
) -> Result<()> {
    ensure!(
        terminal,
        "remote approval requires terminal stdin, stdout, and stderr"
    );
    writeln!(
        writer,
        "Remote app approval (HTTPS, not a signed or verified publisher):"
    )?;
    writeln!(writer, "  Source: {}", grant.source.escape_default())?;
    writeln!(writer, "  SHA-256: {}", grant.sha256)?;
    let permissions = grant
        .permissions
        .iter()
        .map(|p| p.name())
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        writer,
        "  Permissions: {}",
        if permissions.is_empty() {
            "none"
        } else {
            &permissions
        }
    )?;
    write!(writer, "Run this code with these permissions? [y/N] ")?;
    writer.flush()?;
    let mut answer = Vec::new();
    // EOF is never consent, even after an unterminated 'yes'.
    for _ in 0..16 {
        let mut byte = [0];
        if reader.read(&mut byte)? == 0 {
            break;
        }
        answer.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    ensure!(
        matches!(answer.as_slice(), b"y\n" | b"yes\n" | b"y\r\n" | b"yes\r\n"),
        "remote app approval denied"
    );
    Ok(())
}

impl Approval for TerminalApproval {
    fn approve(&mut self, grant: &Grant) -> Result<()> {
        let terminal =
            io::stdin().is_terminal() && io::stdout().is_terminal() && io::stderr().is_terminal();
        ask(
            &mut io::stdin().lock(),
            &mut io::stderr().lock(),
            terminal,
            grant,
        )
    }
}

async fn prepare(
    client: &impl Download,
    root: &Path,
    approval: &mut impl Approval,
    input: &str,
) -> Result<App> {
    let source = source_url(input)?;
    let source_id = hash(source.as_str().as_bytes());
    let bytes = client.get(&source, MANIFEST_LIMIT).await?;
    ensure!(bytes.len() <= MANIFEST_LIMIT, "manifest exceeds size limit");
    let manifest: Manifest =
        serde_json::from_str(std::str::from_utf8(&bytes).context("manifest is not UTF-8")?)
            .map_err(|error| {
                anyhow::anyhow!(
                    "invalid remote manifest: {}",
                    error.to_string().escape_default()
                )
            })?;
    ensure!(
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "sha256 must be a 64-character hex digest"
    );
    let target = bundle_url(&source, &manifest.entry)?;
    let mut permissions = manifest.permissions;
    permissions.sort();
    permissions.dedup();
    let grant = Grant {
        source: source.to_string(),
        sha256: manifest.sha256.to_ascii_lowercase(),
        permissions,
    };
    let cache = cache::Cache::open(root)?;
    let bundle_key = format!("bundle-{}", grant.sha256);
    let bundle_bytes = match cache.read(&bundle_key, BUNDLE_LIMIT)? {
        Some(bytes) => bytes,
        None => client.get(&target, BUNDLE_LIMIT).await?,
    };
    ensure!(
        bundle_bytes.len() <= BUNDLE_LIMIT,
        "bundle exceeds size limit"
    );
    ensure!(
        hash(&bundle_bytes) == grant.sha256,
        "bundle SHA-256 mismatch (download or cache tampering)"
    );
    let bundle = String::from_utf8(bundle_bytes).context("bundle is not UTF-8")?;
    let grant_key = format!("grant-{source_id}");
    let previous = cache.read(&grant_key, MANIFEST_LIMIT)?;
    let approved = previous
        .as_deref()
        .and_then(|bytes| serde_json::from_slice::<Grant>(bytes).ok());
    if approved.as_ref() != Some(&grant) {
        approval.approve(&grant)?;
        // No grant is persisted on denial, failed download, hash mismatch, or invalid UTF-8.
        cache.write(&bundle_key, bundle.as_bytes())?;
        cache.write(&grant_key, &serde_json::to_vec(&grant)?)?;
    } else {
        // Restore a missing, freshly hash-checked bundle without prompting again.
        cache.write(&bundle_key, bundle.as_bytes())?;
    }
    Ok(App {
        namespace: format!("remote-{source_id}"),
        permissions: grant
            .permissions
            .iter()
            .map(|p| p.name().to_owned())
            .collect(),
        bundle,
    })
}

pub(crate) async fn load(input: &str) -> Result<App> {
    let root = dirs_next::cache_dir()
        .context("no platform cache directory")?
        .join("tela-remote-apps");
    prepare(&HttpDownload::new()?, &root, &mut TerminalApproval, input).await
}
