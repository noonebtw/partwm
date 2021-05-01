use log::{debug, error, info, trace, warn};
use log4rs::{
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};

mod clients;
mod state;
mod util;
mod xlib;

#[allow(dead_code)]
unsafe extern "C" fn xlib_error_handler(
    _dpy: *mut x11::xlib::Display,
    ee: *mut x11::xlib::XErrorEvent,
) -> std::os::raw::c_int {
    let err = ee.as_ref().unwrap();

    if err.error_code == x11::xlib::BadWindow
        || err.error_code == x11::xlib::BadDrawable
        || err.error_code == x11::xlib::BadAccess
        || err.error_code == x11::xlib::BadMatch
    {
        0
    } else {
        error!(
            "wm: fatal error:\nrequest_code: {}\nerror_code: {}",
            err.request_code, err.error_code
        );
        std::process::exit(1);
    }
}

fn init_logger() {
    let encoder = Box::new(PatternEncoder::new(
        "{d(%Y-%m-%d %H:%M:%S %Z)(utc)} │ {({M}::{f}:{L}):>25} │ {h({l:>5})} │ {m}{n}",
    ));

    let stdout = ConsoleAppender::builder().encoder(encoder.clone()).build();

    let home = dirs::home_dir().expect("Failed to get $HOME env var.");

    let logfile = FileAppender::builder()
        //.encoder(Box::new(PatternEncoder::default()))
        .encoder(encoder)
        .build(home.join(".local/portlights.log"))
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        //.appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(log::LevelFilter::Info),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}

fn main() {
    init_logger();

    log_prologue();

    state::WindowManager::new().init().run();
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
