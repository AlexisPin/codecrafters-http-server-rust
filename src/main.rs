use std::collections::HashMap;
use std::io::Write;

use clap::Parser;
use clap::{arg, command};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const CRLF: &str = "\r\n";
const SUPPORT_ENCODING: [&str; 1] = ["gzip"];

#[derive(Debug)]
enum Method {
    GET,
    POST,
    PUT,
    DELETE,
}

impl Method {
    fn from_str(method: &str) -> Self {
        match method {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "DELETE" => Method::DELETE,
            _ => panic!("Unhandeled method: {}", method),
        }
    }
}

#[derive(Debug)]
struct Request {
    method: Method,
    path: Vec<String>,
    headers: HashMap<String, String>,
    body: String,
}
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    directory: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Args::parse();

    let directory = cli.directory;

    let listener = TcpListener::bind("127.0.0.1:4221").await?;
    loop {
        let directory = directory.clone();
        let (socket, adrr) = listener.accept().await?;
        println!("Accepted connection from: {}", adrr);
        tokio::spawn(async move { handle_connection(socket, directory).await });
    }
}

fn parse_request(buffer: &[u8]) -> anyhow::Result<Request> {
    let request = String::from_utf8(buffer.to_vec())?;

    let mut headers = HashMap::new();
    let request = request.trim();
    let mut parts = request.split(CRLF);

    let start_line = parts.next().unwrap_or_default();
    let mut start_parts = start_line.split_whitespace();

    let method = Method::from_str(start_parts.next().unwrap_or_default());
    let path = start_parts.next().unwrap_or_default();
    let path = path.splitn(3, '/').map(|s| s.to_string()).skip(1).collect();

    let mut body = String::new();
    while let Some(line) = parts.next() {
        if line.is_empty() {
            body = parts.to_owned().collect::<Vec<&str>>().join(CRLF);
            break;
        }
        let mut parts = line.split(": ");
        headers.insert(
            parts.next().unwrap_or_default().to_string(),
            parts.next().unwrap_or_default().to_string(),
        );
    }

    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn response_with_content(content: &str, headers: &str, code: u8) -> String {
    format!(
        "HTTP/1.1 {} OK\r\n{}\r\nContent-Length: {}\r\n\r\n{}",
        code,
        headers,
        content.len(),
        content
    )
}

fn response_with_404() -> String {
    "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string()
}

fn handle_get_files_request(directory: String, filename: &str) -> anyhow::Result<String> {
    let path = std::path::Path::new(&directory).join(filename);

    let response = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let headers = "Content-Type: application/octet-stream";
        response_with_content(&content, headers, 200)
    } else {
        response_with_404()
    };

    Ok(response)
}

fn handle_post_files_request(
    directory: String,
    filename: &str,
    body: String,
) -> anyhow::Result<String> {
    let path = std::path::Path::new(&directory).join(filename);

    let response = if path.exists() {
        response_with_404()
    } else {
        let mut file = std::fs::File::create(path)?;
        file.write_all(body.as_bytes())?;
        let headers = "Content-Type: text/plain";
        response_with_content("Created", headers, 201)
    };

    Ok(response)
}

async fn handle_connection(mut stream: TcpStream, directory: Option<String>) -> anyhow::Result<()> {
    let mut buffer = [0; 1024];
    let len = stream.read(&mut buffer).await?;
    let request = parse_request(&buffer[..len])?;

    match request.method {
        Method::GET => match request.path.get(0).unwrap().as_str() {
            "" => {
                let response = format!("HTTP/1.1 200 OK\r\n\r\n");
                stream.write_all(response.as_bytes()).await?;
            }
            "echo" => {
                let mut headers = "Content-Type: text/plain\r\n".to_string();
                if let Some(accept_encoding) = request.headers.get("Accept-Encoding") {
                    if SUPPORT_ENCODING.contains(&accept_encoding.as_str()) {
                        headers.push_str(
                            format!("Content-Encoding: {}\r\n", accept_encoding).as_str(),
                        );
                    }
                }
                let response = response_with_content(
                    request.path.get(1).unwrap_or(&"".to_string()),
                    &headers,
                    200,
                );
                stream.write_all(response.as_bytes()).await?;
            }
            "user-agent" => {
                let response = response_with_content(
                    request.headers.get("User-Agent").unwrap(),
                    "Content-Type: text/plain",
                    200,
                );
                stream.write_all(response.as_bytes()).await?;
            }
            "files" => {
                let response = match directory {
                    Some(directory) => handle_get_files_request(
                        directory,
                        request.path.get(1).unwrap_or(&"".to_string()),
                    )?,
                    None => response_with_404(),
                };
                stream.write_all(response.as_bytes()).await?;
            }
            _ => stream.write_all(response_with_404().as_bytes()).await?,
        },
        Method::POST => match request.path.get(0).unwrap().as_str() {
            "files" => {
                let response = match directory {
                    Some(directory) => handle_post_files_request(
                        directory,
                        request.path.get(1).unwrap_or(&"".to_string()),
                        request.body,
                    )?,
                    None => response_with_404(),
                };
                stream.write_all(response.as_bytes()).await?;
            }
            _ => stream.write_all(response_with_404().as_bytes()).await?,
        },
        _ => stream.write_all(response_with_404().as_bytes()).await?,
    }

    Ok(())
}
