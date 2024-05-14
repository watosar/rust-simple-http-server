use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    println!("current dir: {}", env::current_dir().unwrap().display());

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read_exact(&mut buffer).unwrap();

    let get_header = b"GET / HTTP/1.1\r\n";
    let (http_status, filename) = if buffer.starts_with(get_header) {
        ("200 OK", "hello.html")
    } else {
        ("404 NOT FOUND", "404.html")
    };

    let mut file = File::open(format!("{}{}", "html/", filename)).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let status_line = format!("HTTP/1.1 {}\r\n\r\n", http_status);
    let response = format!("{}{}", status_line, contents);

    stream.write_all(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
