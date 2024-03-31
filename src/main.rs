use std::collections::HashMap;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221").await?;

    loop {
        let (socket, adrr) = listener.accept().await?;
        println!("Accepted connection from: {}", adrr);
        tokio::spawn(async move { handle_connection(socket).await });
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

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
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
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    request.path.get(1).unwrap().len(),
                    request.path.get(1).unwrap()
                );
                stream.write_all(response.as_bytes()).await?;
            }
            "user-agent" => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    request.headers.get("User-Agent").unwrap().len(),
                    request.headers.get("User-Agent").unwrap()
                );
                stream.write_all(response.as_bytes()).await?;
            }
            _ => {
                let response = format!("HTTP/1.1 404 NOT FOUND\r\n\r\n");
                stream.write_all(response.as_bytes()).await?;
            }
        },
        _ => {}
    }

    Ok(())
}
