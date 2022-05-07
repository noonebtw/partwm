use std::io::Read;

use log::{debug, error, info, trace, warn};
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};

use wm::state::WMConfig;

fn init_logger() {
    let encoder = Box::new(PatternEncoder::new(
        "{d(%Y-%m-%d %H:%M:%S %Z)(utc)} │ {({M}::{f}:{L}):>25} │ {h({l:>5})} │ {m}{n}",
    ));

    let stdout = ConsoleAppender::builder().encoder(encoder.clone()).build();

    let home = dirs::home_dir().expect("Failed to get $HOME env var.");

    let _logfile = FileAppender::builder()
        .encoder(encoder)
        .build(home.join(".local/portlights.log"))
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        //.appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("stdout")
                //.appender("logfile")
                .build(log::LevelFilter::Debug),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}

fn main() {
    init_logger();

    log_prologue();

    let mut config_path = std::path::PathBuf::from(env!("HOME"));
    config_path.push(".config/nowm.toml");

    let config = std::fs::File::open(config_path)
        .and_then(|mut file| {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            Ok(content)
        })
        .and_then(|content| Ok(toml::from_str::<WMConfig>(&content)?))
        .unwrap_or_else(|e| {
            warn!("error parsing config file: {}", e);
            info!("falling back to default config.");
            WMConfig::default()
        });

    wm::state::WindowManager::<wm::backends::xlib::XLib>::new(config).run();
}

fn log_prologue() {
    info!("========================================================================");
    info!("Portlights Window Manager");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Warning levels:");
    info!("Info!");
    warn!("Warning!");
    debug!("Debug!");
    error!("Error!");
    trace!("Trace!");
    info!("========================================================================");
}

#[test]
fn test_logger() {
    init_logger();

    log_prologue();
}
