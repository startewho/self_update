/*!
Example updating an executable to the latest version released via GitHub
*/

// For the `cargo_crate_version!` macro
#[macro_use]
extern crate log;
extern crate simplelog;
use simplelog::*;
extern crate update;
use serde::{Deserialize, Serialize};
use std::fs::{self};
use std::path::Path;

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct Setting {
    api_root: Option<String>,
    install_path: Option<String>,
    install_bin: Option<String>,
    retry_time: u32,
    ignore_ver_compare: bool,
    before_cmd:Option<String>,
    after_cmd:Option<String>
}

fn bin_ver(bin: &Path) -> Option<String> {
    use std::process::*;
    let output = if cfg!(target_os = "windows") {
        Command::new(bin)
            .args(&["--version"])
            .output()
            .expect("failed to execute process")
    } else {
        Command::new(bin)
            .arg("--version")
            .arg("")
            .output()
            .expect("failed to execute process")
    };
    use regex::Regex;
    let re = Regex::new(r"\d+\S+").unwrap();
    let msg = if output.status.success() {
        String::from_utf8(output.stdout).unwrap()
    } else {
        String::from_utf8(output.stderr).unwrap()
    };
    let cap = re.captures(&msg).unwrap();
    if cap.len() > 0 {
        Some(cap.get(0).unwrap().as_str().into())
    } else {
        None
    }
}

fn run() -> Result<(), Box<dyn ::std::error::Error>> {
    let file = fs::read("setting.json")?;
    let setting: Setting = serde_json::from_slice(&file)?;
    let api_root = setting.api_root.unwrap_or("http://127.0.0.1".into());
    let path = setting
        .install_path
        .unwrap_or("D:\\Server\\CloudAgent".into());
    let ignore_ver = setting.ignore_ver_compare;
    let bin_name = setting.install_bin.unwrap_or("CloudAgent.exe".into());
    let bin_dir = Path::new(&path);
    if !bin_dir.is_dir() {
        info!("Create Dir:{:?}", &bin_dir);
        fs::create_dir_all(&bin_dir)?;
    }
    info!("Update Dir:{:?}", &bin_dir);
    let bin_path = bin_dir.join(&bin_name);
    let ver = bin_ver(&bin_path).unwrap();
    let status = update::backends::cloud::Update::configure()
        .name("Agent")
        .custom_url(&api_root)
        .bin_name(&bin_name)
        .no_confirm(true)
        .ignore_ver_compare(ignore_ver)
        .show_download_progress(true)
        .bin_install_path(&bin_dir)
        .before_cmd(setting.before_cmd.as_ref().unwrap())
        .after_cmd(setting.after_cmd.as_ref().unwrap())
        //.target_version_tag("v9.9.10")
        //.show_output(false)
        //.no_confirm(true)
        //
        // For private repos, you will need to provide a GitHub auth token
        // **Make sure not to bake the token into your app**; it is recommended
        // you obtain it via another mechanism, such as environment variables
        // or prompting the user for input
        //.auth_token(env!("DOWNLOAD_AUTH_TOKEN"))
        .current_version(&ver)
        .build()?
        .update()?;
    println!("Updated version: `{}`!", status.version());
    Ok(())
}

pub fn main() -> std::io::Result<()> {
    use std::env;
    use std::fs::OpenOptions;

    let path = env::current_exe()?.parent().unwrap().join("log.log");

    let mut build = ConfigBuilder::new();
    let config = build.set_time_to_local(true).build();

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            config.clone(),
            OpenOptions::new()
                .read(true)
                .write(true)
                .append(true)
                .create(true)
                .open(&path)
                .unwrap(),
        ),
    ])
    .unwrap();

    if let Err(e) = run() {
        error!("[ERROR] {:?}", e);
        ::std::process::exit(1);
    }
    let mut redkey = String::new();
    std::io::stdin().read_line(&mut redkey)?;
    info!("{}", redkey);
    Ok(())
}
