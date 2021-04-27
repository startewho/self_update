/*!
Cloud releases
*/

use indicatif::ProgressStyle;
use reqwest::{self, header};
use serde::{Deserialize, Serialize};
use std::env::{self, consts::EXE_SUFFIX};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    errors::*,
    get_target,
    update::{Release, ReleaseAsset, ReleaseUpdate},
};

fn from_cloud(soft: &Soft, root_url: &str) -> Result<Release> {
    let mut assets = Vec::new();
    assets.push(ReleaseAsset {
        name: soft.name.clone().unwrap().into(),
        download_url: String::from(format!(
            "{}/api/binaryfile/download?id={}",
            root_url, soft.binary_id
        )),
    });
    Ok(Release {
        name: soft.name.clone().unwrap().into(),
        version: soft.version.clone().unwrap().into(),
        date: soft.create_time.as_ref().unwrap_or(&"".to_string()).clone(),
        body: None,
        assets: assets,
    })
}

/// `ReleaseList` Builder

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetResponse<T> {
    is_success: bool,
    content: T,
    error_mesg: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Soft {
    id: i64,
    binary_id: i64,
    name: Option<String>,
    hash: Option<String>,
    version: Option<String>,
    create_time: Option<String>,
}

/// `ReleaseList` Builder
#[derive(Clone, Debug)]
pub struct ReleaseListBuilder {
    name: Option<String>,
    target: Option<String>,
    auth_token: Option<String>,
    custom_url: Option<String>,
}
impl ReleaseListBuilder {
    pub fn with_name(&mut self, name: &str) -> &mut Self {
        self.name = Some(name.to_owned());
        self
    }

    pub fn custom_url(&mut self, url: &str) -> &mut Self {
        self.custom_url = Some(url.to_owned());
        self
    }

    /// Set the optional arch `target` name, used to filter available releases
    pub fn with_target(&mut self, target: &str) -> &mut Self {
        self.target = Some(target.to_owned());
        self
    }

    /// Set the optional github url, e.g. for a github enterprise installation.
    /// The url should provide the path to your API endpoint and end without a trailing slash,
    /// for example `https://api.github.com` or `https://github.mycorp.com/api/v3`
    pub fn with_url(&mut self, url: &str) -> &mut Self {
        self.custom_url = Some(url.to_owned());
        self
    }

    /// Set the authorization token, used in requests to the github api url
    ///
    /// This is to support private repos where you need a GitHub auth token.
    /// **Make sure not to bake the token into your app**; it is recommended
    /// you obtain it via another mechanism, such as environment variables
    /// or prompting the user for input
    pub fn auth_token(&mut self, auth_token: &str) -> &mut Self {
        self.auth_token = Some(auth_token.to_owned());
        self
    }

    /// Verify builder args, returning a `ReleaseList`
    pub fn build(&self) -> Result<ReleaseList> {
        Ok(ReleaseList {
            name: self.name.clone(),
            target: self.target.clone(),
            auth_token: self.auth_token.clone(),
            custom_url: self.custom_url.clone(),
        })
    }
}

/// `ReleaseList` provides a builder api for querying a GitHub repo,
/// returning a `Vec` of available `Release`s
#[derive(Clone, Debug)]
pub struct ReleaseList {
    name: Option<String>,
    target: Option<String>,
    auth_token: Option<String>,
    custom_url: Option<String>,
}
impl ReleaseList {
    /// Initialize a ReleaseListBuilder
    pub fn configure() -> ReleaseListBuilder {
        ReleaseListBuilder {
            name: None,
            target: None,
            auth_token: None,
            custom_url: None,
        }
    }

    /// Retrieve a list of `Release`s.
    /// If specified, filter for those containing a specified `target`
    pub fn fetch(self) -> Result<Vec<Release>> {
        set_ssl_vars!();
        let api_url = format!(
            "{}/api/soft/getlist?type=2",
            self.custom_url
                .as_ref()
                .unwrap_or(&"http:127.0.0.1".to_string())
        );

        let releases = self.fetch_releases(&api_url)?;
        let releases = match self.target {
            None => releases,
            Some(ref target) => releases
                .into_iter()
                .filter(|r| r.has_target_asset(target))
                .collect::<Vec<_>>(),
        };
        Ok(releases)
    }

    fn fetch_releases(&self, url: &str) -> Result<Vec<Release>> {
        let resp = reqwest::blocking::Client::new()
            .get(url)
            .headers(api_headers(&self.auth_token)?)
            .send()?;
        if !resp.status().is_success() {
            bail!(
                Error::Network,
                "api request failed with status: {:?} - for: {:?}",
                resp.status(),
                url
            )
        }
        let json = resp.json::<NetResponse<Vec<Soft>>>()?;
        if json.is_success && (json.content.len() > 0) {
            return json
                .content
                .iter()
                .map(|s| from_cloud(s, &self.custom_url.as_ref().unwrap()))
                .collect::<Result<Vec<Release>>>();
        }
        bail!(Error::Release, "Not found Release")
    }
}

/// `github::Update` builder
///
/// Configure download and installation from
/// `https://api.github.com/repos/<repo_owner>/<repo_name>/releases/latest`
#[derive(Debug)]
pub struct UpdateBuilder {
    name: Option<String>,
    target: Option<String>,
    bin_name: Option<String>,
    bin_install_path: Option<PathBuf>,
    bin_path_in_archive: Option<PathBuf>,
    show_download_progress: bool,
    show_output: bool,
    no_confirm: bool,
    ignore_ver_compare: bool,
    current_version: Option<String>,
    target_version: Option<String>,
    progress_style: Option<ProgressStyle>,
    auth_token: Option<String>,
    custom_url: Option<String>,
}

impl UpdateBuilder {
    /// Initialize a new builder
    pub fn new() -> Self {
        Default::default()
    }
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = Some(name.to_owned());
        self
    }

    pub fn custom_url(&mut self, url: &str) -> &mut Self {
        self.custom_url = Some(url.to_owned());
        self
    }

    /// Set the update builder's ignore ver compare.
    pub fn ignore_ver_compare(&mut self, ignore_ver_compare: bool) -> &mut Self {
        self.ignore_ver_compare = ignore_ver_compare;
        self
    }

    /// Set the current app version, used to compare against the latest available version.
    /// The `cargo_crate_version!` macro can be used to pull the version from your `Cargo.toml`
    pub fn current_version(&mut self, ver: &str) -> &mut Self {
        self.current_version = Some(ver.to_owned());
        self
    }

    /// Set the target version tag to update to. This will be used to search for a release
    /// by tag name:
    /// `/repos/:owner/:repo/releases/tags/:tag`
    ///
    /// If not specified, the latest available release is used.
    pub fn target_version_tag(&mut self, ver: &str) -> &mut Self {
        self.target_version = Some(ver.to_owned());
        self
    }

    /// Set the target triple that will be downloaded, e.g. `x86_64-unknown-linux-gnu`.
    ///
    /// If unspecified, the build target of the crate will be used
    pub fn target(&mut self, target: &str) -> &mut Self {
        self.target = Some(target.to_owned());
        self
    }

    /// Set the exe's name. Also sets `bin_path_in_archive` if it hasn't already been set.
    ///
    /// This method will append the platform specific executable file suffix
    /// (see `std::env::consts::EXE_SUFFIX`) to the name if it's missing.
    pub fn bin_name(&mut self, name: &str) -> &mut Self {
        let raw_bin_name = format!("{}{}", name.trim_end_matches(EXE_SUFFIX), EXE_SUFFIX);
        self.bin_name = Some(raw_bin_name.clone());
        if self.bin_path_in_archive.is_none() {
            self.bin_path_in_archive = Some(PathBuf::from(raw_bin_name));
        }
        self
    }

    /// Set the installation path for the new exe, defaults to the current
    /// executable's path
    pub fn bin_install_path<A: AsRef<Path>>(&mut self, bin_install_path: A) -> &mut Self {
        self.bin_install_path = Some(PathBuf::from(bin_install_path.as_ref()));
        self
    }

    /// Set the path of the exe inside the release tarball. This is the location
    /// of the executable relative to the base of the tar'd directory and is the
    /// path that will be copied to the `bin_install_path`. If not specified, this
    /// will default to the value of `bin_name`. This only needs to be specified if
    /// the path to the binary (from the root of the tarball) is not equal to just
    /// the `bin_name`.
    ///
    /// # Example
    ///
    /// For a tarball `myapp.tar.gz` with the contents:
    ///
    /// ```shell
    /// myapp.tar/
    ///  |------- bin/
    ///  |         |--- myapp  # <-- executable
    /// ```
    ///
    /// The path provided should be:
    ///
    /// ```
    /// # use self_update::backends::github::Update;
    /// # fn run() -> Result<(), Box<::std::error::Error>> {
    /// Update::configure()
    ///     .bin_path_in_archive("bin/myapp")
    /// #   .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn bin_path_in_archive(&mut self, bin_path: &str) -> &mut Self {
        self.bin_path_in_archive = Some(PathBuf::from(bin_path));
        self
    }

    /// Toggle download progress bar, defaults to `off`.
    pub fn show_download_progress(&mut self, show: bool) -> &mut Self {
        self.show_download_progress = show;
        self
    }

    /// Toggle download progress bar, defaults to `off`.
    pub fn set_progress_style(&mut self, progress_style: ProgressStyle) -> &mut Self {
        self.progress_style = Some(progress_style);
        self
    }

    /// Toggle update output information, defaults to `true`.
    pub fn show_output(&mut self, show: bool) -> &mut Self {
        self.show_output = show;
        self
    }

    /// Toggle download confirmation. Defaults to `false`.
    pub fn no_confirm(&mut self, no_confirm: bool) -> &mut Self {
        self.no_confirm = no_confirm;
        self
    }

    /// Set the authorization token, used in requests to the github api url
    ///
    /// This is to support private repos where you need a GitHub auth token.
    /// **Make sure not to bake the token into your app**; it is recommended
    /// you obtain it via another mechanism, such as environment variables
    /// or prompting the user for input
    pub fn auth_token(&mut self, auth_token: &str) -> &mut Self {
        self.auth_token = Some(auth_token.to_owned());
        self
    }

    /// Confirm config and create a ready-to-use `Update`
    ///
    /// * Errors:
    ///     * Config - Invalid `Update` configuration
    pub fn build(&self) -> Result<Box<dyn ReleaseUpdate>> {
        let bin_install_path = if let Some(v) = &self.bin_install_path {
            v.clone()
        } else {
            env::current_exe()?
        };

        Ok(Box::new(Update {
            name: if let Some(ref name) = self.name {
                name.to_owned()
            } else {
                bail!(Error::Config, "`bin_name` required")
            },
            target: self
                .target
                .as_ref()
                .map(|t| t.to_owned())
                .unwrap_or_else(|| get_target().to_owned()),
            bin_name: if let Some(ref name) = self.bin_name {
                name.to_owned()
            } else {
                bail!(Error::Config, "`bin_name` required")
            },
            bin_install_path,
            bin_path_in_archive: if let Some(ref path) = self.bin_path_in_archive {
                path.to_owned()
            } else {
                bail!(Error::Config, "`bin_path_in_archive` required")
            },
            current_version: if let Some(ref ver) = self.current_version {
                ver.to_owned()
            } else {
                bail!(Error::Config, "`current_version` required")
            },
            target_version: self.target_version.as_ref().map(|v| v.to_owned()),
            show_download_progress: self.show_download_progress,
            progress_style: self.progress_style.clone(),
            show_output: self.show_output,
            no_confirm: self.no_confirm,
            ignore_ver_compare: self.ignore_ver_compare,
            auth_token: self.auth_token.clone(),
            custom_url: self.custom_url.clone(),
        }))
    }
}

/// Updates to a specified or latest release distributed via GitHub
#[derive(Debug)]
pub struct Update {
    name: String,
    target: String,
    current_version: String,
    target_version: Option<String>,
    bin_name: String,
    bin_install_path: PathBuf,
    bin_path_in_archive: PathBuf,
    show_download_progress: bool,
    ignore_ver_compare: bool,
    show_output: bool,
    no_confirm: bool,
    progress_style: Option<ProgressStyle>,
    auth_token: Option<String>,
    custom_url: Option<String>,
}
impl Update {
    /// Initialize a new `Update` builder
    pub fn configure() -> UpdateBuilder {
        UpdateBuilder::new()
    }
}

impl ReleaseUpdate for Update {
    fn get_latest_release(&self) -> Result<Release> {
        self.get_release_version("")
    }

    fn get_release_version(&self, ver: &str) -> Result<Release> {
        set_ssl_vars!();
        let api_url = format!(
            "{}/api/soft/getver?type=2&ver={}",
            self.custom_url
                .as_ref()
                .unwrap_or(&"http://127.0.0.1:5000".to_string()),
            ver
        );

        let resp = reqwest::blocking::Client::new()
            .get(&api_url)
            .headers(api_headers(&self.auth_token)?)
            .send()?;
        if !resp.status().is_success() {
            bail!(
                Error::Network,
                "api request failed with status: {:?} - for: {:?}",
                resp.status(),
                api_url
            )
        }
        let json = resp.json::<NetResponse<Soft>>()?;
        if json.is_success {
            Ok(from_cloud(&json.content, &self.custom_url.as_ref().unwrap()).unwrap())
        } else {
            bail!(Error::Release, "can not get Last relesae",)
        }
    }

    fn current_version(&self) -> String {
        self.current_version.to_owned()
    }

    fn target(&self) -> String {
        self.target.clone()
    }

    fn target_version(&self) -> Option<String> {
        self.target_version.clone()
    }

    fn bin_name(&self) -> String {
        self.bin_name.clone()
    }

    fn bin_install_path(&self) -> PathBuf {
        self.bin_install_path.clone()
    }

    fn bin_path_in_archive(&self) -> PathBuf {
        self.bin_path_in_archive.clone()
    }

    fn show_download_progress(&self) -> bool {
        self.show_download_progress
    }
    fn ignore_ver_compare(&self) -> bool {
        return self.ignore_ver_compare;
    }

    fn show_output(&self) -> bool {
        self.show_output
    }

    fn no_confirm(&self) -> bool {
        self.no_confirm
    }

    fn idty_target_platform(&self) -> bool {
        return false;
    }

    fn all_replce(&self) -> bool {
        return true;
    }

    /// action before the update start
    fn before_update(&self) -> () {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(&["/C", "sc stop CloudAgent"])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("sh")
                .arg("-c")
                .arg("sv stop CloudAgent")
                .output()
                .expect("failed to execute process")
        };
        let out = String::from_utf8(output.stdout).unwrap();
        info!(
            "Before update:{:?},Status:{},Result:{}",
            self.bin_install_path(),
            output.status,
            out
        );
    }

    ///action after the update have finished
    fn after_update(&self) -> () {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(&["/C", "sc start CloudAgent"])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("sh")
                .arg("-c")
                .arg("sv stop CloudAgent")
                .output()
                .expect("failed to execute process")
        };
        let out = String::from_utf8(output.stdout).unwrap();
        info!(
            "After update:{:?},Status:{},Result:{}",
            self.bin_install_path(),
            output.status,
            out
        );
    }

    fn progress_style(&self) -> Option<ProgressStyle> {
        self.progress_style.clone()
    }

    fn auth_token(&self) -> Option<String> {
        self.auth_token.clone()
    }

   
}

impl Default for UpdateBuilder {
    fn default() -> Self {
        Self {
            name: None,
            target: None,
            bin_name: None,
            bin_install_path: None,
            bin_path_in_archive: None,
            show_download_progress: false,
            show_output: true,
            ignore_ver_compare: true,
            no_confirm: false,
            current_version: None,
            target_version: None,
            progress_style: None,
            auth_token: None,
            custom_url: None,
        }
    }
}

fn api_headers(auth_token: &Option<String>) -> Result<header::HeaderMap> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        "rust-reqwest/self-update"
            .parse()
            .expect("github invalid user-agent"),
    );

    if let Some(token) = auth_token {
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", token)
                .parse()
                .map_err(|err| Error::Config(format!("Failed to parse auth token: {}", err)))?,
        );
    };

    Ok(headers)
}
