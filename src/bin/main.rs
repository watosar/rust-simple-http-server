use log::{error, info, warn};
use regex::Regex;
use simple_http_server::lib_server_impl::handle_connection;
use simple_http_server::lib_thread_pool::ThreadPool;
use std::env;
use std::fs::File;
use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;

static CONFIG_PATTERN: &str = r"config\s+rust-simple-http-server\s+'main'
\s+list\s+listen_http\s'(?<addr>\d+\.\d+\.\d+.\d:\d+)'
(?:\s+option\shome\s'(?<home>.+?)')?";
fn get_config() -> (String, PathBuf) {
    let conf_file_path = "/etc/config/rust-simple-http-server";
    let default_listen = "0.0.0.0:8000";
    let default_home = "/var/www";

    let mut f = match File::open(conf_file_path) {
        Err(msg) => {
            warn!("could not found config file {conf_file_path} because of {msg}.");
            return (String::from(default_listen), PathBuf::from(default_home));
        }
        Ok(file) => file,
    };

    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");

    let re = Regex::new(CONFIG_PATTERN).unwrap();
    let Some(caps) = re.captures(&contents) else {
        warn!("could not parse config from {}.", conf_file_path);
        return (String::from(default_listen), PathBuf::from(default_home));
    };
    let listen_http = &caps["addr"];
    let home = &caps["home"];
    if home.is_empty() {
        return (String::from(listen_http), PathBuf::from(default_home));
    }

    (String::from(listen_http), PathBuf::from(home))
}

fn main() {
    pretty_env_logger::init_timed();
    info!("start server");

    let (listen_addr, home) = get_config();

    info!("start listen: {listen_addr}");
    let listener = TcpListener::bind(listen_addr).unwrap();
    let pool = ThreadPool::new(4);

    info!("cd {}", home.display());
    env::set_current_dir(home.clone()).unwrap_or_else(|_err| {
        warn!(
            "failed to change dir to {} because of {_err}",
            home.display()
        );
    });
    info!("current dir: {}", env::current_dir().unwrap().display());

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        pool.execute(|| {
            if let Err(msg) = handle_connection(stream) {
                error!("{msg}");
            };
        });
    }
    // 閉じます
    println!("Shutting down.");
}
