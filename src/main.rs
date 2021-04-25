/*!
Example updating an executable to the latest version released via GitHub
*/

// For the `cargo_crate_version!` macro
#[macro_use] extern crate log;
extern crate simplelog;
use simplelog::*;
extern crate self_update;
use std::fs::{self,*};
use std::path::Path;
use serde::{Deserialize, Serialize};


#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    api_root:Option<String>,
    install_path:Option<String>,
    retry_time:u32,
}


fn run() -> Result<(), Box<dyn ::std::error::Error>> {
    let file= fs::read("setting.json")?;
    let setting:Setting= serde_json::from_slice(&file)?;
    let api_root=setting.api_root.unwrap_or("http://106.14.207.124".into());
    let path=setting.install_path.unwrap_or("D:\\Server".into());
    let bin_path=Path::new(&path);
    if !bin_path.is_dir()
    {
        info!("Create Dir:{:?}",&bin_path);
        fs::create_dir(&bin_path)?;
    }
    info!("Update Dir:{:?}",&bin_path);
    let status = self_update::backends::cloud::Update::configure()
        .name("Agent")
        .custom_url(&api_root)
        .bin_name("CloudAgent")
        .no_confirm(true)
        .show_download_progress(true)
        .bin_install_path(&bin_path)
        //.target_version_tag("v9.9.10")
        //.show_output(false)
        //.no_confirm(true)
        //
        // For private repos, you will need to provide a GitHub auth token
        // **Make sure not to bake the token into your app**; it is recommended
        // you obtain it via another mechanism, such as environment variables
        // or prompting the user for input
        //.auth_token(env!("DOWNLOAD_AUTH_TOKEN"))
        .current_version("1.0.1")
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}

pub fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create("info.log").unwrap()),
        ]
    ).unwrap();
    if let Err(e) = run() {
        println!("[ERROR] {}", e);
        ::std::process::exit(1);
    }
}
