use std::collections::HashMap;

use clap::Parser;
use clap::{arg, command};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
struct Request {
    method: String,
    path: Vec<String>,
    headers: HashMap<String, String>,
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
    let mut parts = request.split_whitespace();

    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let path = path.splitn(3, '/').map(|s| s.to_string()).skip(1).collect();

    let user_agent = parts
        .skip_while(|&s| s != "User-Agent:")
        .skip(1)
        .next()
        .unwrap_or_default()
        .to_string();

    headers.insert("User-Agent".to_string(), user_agent);

    Ok(Request {
        method,
        path,
        headers,
    })
}

fn respond_with_content(content: &str, content_type: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
        content_type,
        content.len(),
        content
    )
}

fn respond_with_404() -> String {
    "HTTP/1.1 404 NOT FOUND\r\n\r\n".to_string()
}

fn handle_files_request(directory: String, filename: &str) -> anyhow::Result<String> {
    let path = std::path::Path::new(&directory).join(filename);

    let response = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        respond_with_content(&content, "application/octet-stream")
    } else {
        respond_with_404()
    };

    Ok(response)
}

async fn handle_connection(mut stream: TcpStream, directory: Option<String>) -> anyhow::Result<()> {
    let mut buffer = [0; 1024];
    let len = stream.read(&mut buffer).await?;
    let request = parse_request(&buffer[..len])?;

    match request.method.as_str() {
        "GET" => match request.path.get(0).unwrap().as_str() {
            "" => {
                let response = format!("HTTP/1.1 200 OK\r\n\r\n");
                stream.write_all(response.as_bytes()).await?;
            }
            "echo" => {
                let response = respond_with_content(request.path.get(1).unwrap_or(&"".to_string()), "text/plain");
                stream.write_all(response.as_bytes()).await?;
            }
            "user-agent" => {
                let response =
                    respond_with_content(request.headers.get("User-Agent").unwrap(), "text/plain");
                stream.write_all(response.as_bytes()).await?;
            }
            "files" => {
                let response = match directory {
                    Some(directory) => handle_files_request(
                        directory,
                        request.path.get(1).unwrap_or(&"".to_string()),
                    )?,
                    None => respond_with_404(),
                };
                stream.write_all(response.as_bytes()).await?;
            }
            _ => stream.write_all(respond_with_404().as_bytes()).await?,
        },
        _ => {}
    }

    Ok(())
}
