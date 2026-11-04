//! Small, synchronous pCloud HTTP JSON adapter used only during explicit sync.
use std::{collections::BTreeMap, time::Duration};

use reqwest::blocking::{multipart, Client};
use serde_json::Value;

use crate::error::{BeverError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PCloudCredentials {
    pub access_token: String,
    pub hostname: String,
}

impl PCloudCredentials {
    pub fn api_base(&self) -> Result<String> {
        if !matches!(self.hostname.as_str(), "api.pcloud.com" | "eapi.pcloud.com") {
            return Err(BeverError::Invalid(
                "unsupported pCloud API hostname".into(),
            ));
        }
        Ok(format!("https://{}", self.hostname))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    pub name: String,
    pub size: u64,
}

pub struct PCloudClient {
    credentials: PCloudCredentials,
    http: Client,
}

impl PCloudClient {
    pub fn new(credentials: PCloudCredentials) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("Bevernote/0.1")
            .build()?;
        Ok(Self { credentials, http })
    }

    pub fn initialize(&self) -> Result<()> {
        self.get("createfolderifnotexists", &[("path", "/Bevernote")])?;
        self.get("createfolderifnotexists", &[("path", "/Bevernote/items")])?;
        self.get("createfolderifnotexists", &[("path", "/Bevernote/blobs")])?;
        Ok(())
    }

    pub fn list(&self, path: &str) -> Result<Vec<RemoteFile>> {
        let value = self.get("listfolder", &[("path", path)])?;
        let Some(contents) = value
            .get("metadata")
            .and_then(|metadata| metadata.get("contents"))
            .and_then(Value::as_array)
        else {
            return Ok(Vec::new());
        };
        Ok(contents
            .iter()
            .filter(|entry| entry.get("isfolder").and_then(Value::as_bool) == Some(false))
            .filter_map(|entry| {
                Some(RemoteFile {
                    name: entry.get("name")?.as_str()?.to_owned(),
                    size: entry.get("size").and_then(Value::as_u64).unwrap_or(0),
                })
            })
            .collect())
    }

    pub fn download(&self, path: &str) -> Result<Vec<u8>> {
        let value = self.get("getfilelink", &[("path", path)])?;
        let host = value
            .get("hosts")
            .and_then(Value::as_array)
            .and_then(|hosts| hosts.first())
            .and_then(Value::as_str)
            .ok_or_else(|| BeverError::Remote("pCloud returned no download host".into()))?;
        if !host.ends_with(".pcloud.com") {
            return Err(BeverError::Remote(
                "pCloud returned an invalid download host".into(),
            ));
        }
        let remote_path = value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| BeverError::Remote("pCloud returned no download path".into()))?;
        let response = self
            .http
            .get(format!("https://{host}{remote_path}"))
            .send()?
            .error_for_status()?;
        Ok(response.bytes()?.to_vec())
    }

    pub fn upload(&self, path: &str, bytes: Vec<u8>) -> Result<()> {
        let (directory, filename) = path
            .rsplit_once('/')
            .ok_or_else(|| BeverError::Invalid("invalid remote file path".into()))?;
        let directory = if directory.is_empty() { "/" } else { directory };
        let response = self
            .http
            .post(format!("{}/uploadfile", self.credentials.api_base()?))
            .query(&[
                ("access_token", self.credentials.access_token.as_str()),
                ("path", directory),
                ("filename", filename),
                ("nopartial", "1"),
                ("renameifnotexists", "0"),
            ])
            .multipart(
                multipart::Form::new().part(
                    "file",
                    multipart::Part::bytes(bytes)
                        .file_name(filename.to_owned())
                        .mime_str("application/octet-stream")?,
                ),
            )
            .send()?
            .error_for_status()?;
        let value: Value = response.json()?;
        check_result(value).map(|_| ())
    }

    pub fn user_email(&self) -> Result<Option<String>> {
        let value = self.get("userinfo", &[])?;
        Ok(value
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_owned))
    }

    fn get(&self, method: &str, parameters: &[(&str, &str)]) -> Result<Value> {
        let mut owned = BTreeMap::new();
        owned.insert("access_token", self.credentials.access_token.as_str());
        owned.extend(parameters.iter().copied());
        let response = self
            .http
            .get(format!("{}/{method}", self.credentials.api_base()?))
            .query(&owned)
            .send()?
            .error_for_status()?;
        check_result(response.json()?)
    }
}

fn check_result(value: Value) -> Result<Value> {
    let result = value.get("result").and_then(Value::as_i64).unwrap_or(-1);
    if result == 0 || result == 2004 {
        return Ok(value);
    }
    let message = value
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown pCloud error");
    Err(BeverError::Remote(format!("pCloud {result}: {message}")))
}

pub const DEFAULT_CLIENT_ID: &str = "B0DEaAnitD8";
pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:53682/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_official_api_hosts_are_accepted() {
        let credentials = PCloudCredentials {
            access_token: "token".into(),
            hostname: "eapi.pcloud.com".into(),
        };
        assert_eq!(credentials.api_base().unwrap(), "https://eapi.pcloud.com");
        let invalid = PCloudCredentials {
            access_token: "token".into(),
            hostname: "example.com".into(),
        };
        assert!(invalid.api_base().is_err());
    }
}
