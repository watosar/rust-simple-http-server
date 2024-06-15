use hello_ws::ThreadPool;

use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

static PATTERN: &str = r"config\s+myservice\s+'main'
\s+list\s+listen_http\s'(?<addr>\d+\.\d+\.\d+.\d:\d+)'
(?:\s+option\shome\s'(?<home>.+?)')?";
static HTTP_VERSION: &str = "HTTP/1.1";

fn get_config() -> (String, PathBuf) {
    let conf_file_path = "/etc/config/myservice";
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

    let re = Regex::new(PATTERN).unwrap();
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

fn gen_recieve_header(method: &str, dir: &str, version: &str) -> String {
    format!("{method} {dir} {version}\r\n")
}

fn parse_endpoint(method: &str, http_version: &str, buffer: &[u8]) -> Option<String> {
    let re = Regex::new(&gen_recieve_header(method, "/(.*?)", http_version)).unwrap();
    let buff_str = std::str::from_utf8(buffer).unwrap();
    let Some(caps) = re.captures(buff_str) else {
        warn!("could not parse endpoint from {buff_str}.");
        return None;
    };
    Some(String::from(&caps[1]))
}

fn write_header(stream: &mut TcpStream, http_status: &str, content_type: &str) {
    let status_line = format!("{HTTP_VERSION} {http_status}");
    let content_type = if content_type == "text/html" {
        format!("Content-Type: {content_type}; charset=UTF-8")
    } else {
        format!("Content-Type: {content_type};")
    };
    let response = format!("{status_line}\r\n{content_type}\r\nConnection: close\r\n\r\n");
    stream.write_all(response.as_bytes()).unwrap();
}

static EXTENSION_TO_MIMETYPE: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    HashMap::from([
        ("html", "text/html"),
        ("png", "image/png"),
        ("ico", "image/vnd.microsoft.icon"),
    ])
});

fn find<T: std::cmp::PartialEq>(src: &[T], needle: &[T]) -> Option<usize> {
    if src.len() < needle.len() {
        return None;
    }
    (0..src.len() - needle.len() + 1).find(|&i| (src[i..i + needle.len()] == needle[..]))
}

fn read_full_header(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut buffer = Vec::<u8>::new();

    const PROBE_LEN: usize = 14;
    let mut probe = [0; PROBE_LEN];
    if let Ok(size) = stream.read(&mut probe) {
        if size == 0 {
            return None;
        }
        buffer.extend_from_slice(&probe[..size]);
    };

    while {
        let too_long_header = buffer.len() > 2048;
        if too_long_header {
            warn!("recieved too long heaedr");
        }
        !too_long_header
    } && find(
        &buffer[(buffer.len() - std::cmp::min(buffer.len(), PROBE_LEN * 2))..],
        b"\r\n\r\n",
    )
    .is_none()
    {
        match stream.read(&mut probe) {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                buffer.extend_from_slice(&probe[..size]);
            }
            Err(e) => {
                error!("{}", e);
                return None;
            }
        };

        let last_subbuff = &buffer[(buffer.len() - std::cmp::min(buffer.len(), PROBE_LEN * 2))..];
        debug!(
            "request header: {}",
            last_subbuff.iter().fold(String::new(), |mut accum, b| {
                accum += &format!("{b:#04X} ");
                accum
            })
        );
    }

    Some(buffer)
}

fn discard_header(stream: &mut TcpStream) -> Result<(), String> {
    if let Err(msg) = stream.set_nonblocking(true) {
        return Err(format!("{msg}"));
    }

    let mut probe = [0; 1024];
    loop {
        match stream.read(&mut probe) {
            Ok(size) => {
                if size == 0 {
                    break;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                break;
            }
            Err(e) => {
                return Err(format!("{e}"));
            }
        };
    }

    if let Err(msg) = stream.set_nonblocking(false) {
        return Err(format!("{msg}"));
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> Result<String, String> {
    info!("handle connection start.");
    info!("connection from: {}", stream.peer_addr().unwrap());
    let opt_buffer = read_full_header(&mut stream);
    if opt_buffer.is_none() {
        return Err("unexpected EOF.".to_string());
    }
    let buffer = opt_buffer.unwrap();
    match String::from_utf8(buffer.to_vec()) {
        Ok(content) => info!("request header: {}", content),
        Err(err) => return Err(err.to_string()),
    }

    discard_header(&mut stream)?;

    let (http_status, filename) = if buffer.starts_with("GET /".as_bytes()) {
        match parse_endpoint("GET", HTTP_VERSION, &buffer) {
            None => ("404 NOT FOUND", "404.html".to_string()),
            Some(endpoint) => {
                info!("endpoint: {endpoint}");
                (
                    "200 OK",
                    if endpoint.is_empty() {
                        "index.html".to_string()
                    } else {
                        endpoint
                    },
                )
            }
        }
    } else if buffer.starts_with(gen_recieve_header("GET", "/api/sleep", HTTP_VERSION).as_bytes()) {
        thread::sleep(Duration::from_secs(5));
        ("200 OK", "hello.html".to_string())
    } else {
        ("404 NOT FOUND", "404.html".to_string())
    };

    info!("file: {filename}");
    info!("response: {http_status}");

    let filepath = PathBuf::from(format!("{}{}", "./", filename));
    let (http_status, filepath) = if !filepath.exists() {
        ("404 NOT FOUND", PathBuf::from("./404.html"))
    } else {
        (http_status, filepath)
    };

    let file = File::open(filepath.clone());
    if file.is_err() {
        return Err(file.err().unwrap().to_string());
    }
    let mut file = file.unwrap();
    write_header(
        &mut stream,
        http_status,
        EXTENSION_TO_MIMETYPE
            .get(&filepath.extension().and_then(OsStr::to_str).unwrap_or(""))
            .unwrap_or(&""),
    );
    if let Err(err) = std::io::copy(&mut file, &mut stream) {
        return Err(err.to_string());
    };
    if let Err(err) = stream.flush() {
        return Err(err.to_string());
    };

    Ok(http_status.to_string())
}
