use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    version: String,
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

    println!("Request: {}", request);

    let request = request.trim();
    let mut parts = request.split_whitespace();

    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or_default().to_string();

    Ok(Request {
        method,
        path,
        version,
    })
}

async fn handle_connection(mut stream: TcpStream) -> anyhow::Result<()> {
    let mut buffer = [0; 1024];
    let len = stream.read(&mut buffer).await?;
    let request = parse_request(&buffer[..len])?;

    match request.method.as_str() {
        "GET" => {
            match request
                .path
                .as_str()
                .split("/")
                .collect::<Vec<&str>>()
                .as_slice()
            {
                ["", ""] => {
                    let response = format!("HTTP/1.1 200 OK\r\n\r\n");
                    stream.write_all(response.as_bytes()).await?;
                }
                //regex match echo/<string>
                ["", "echo", rest] => {
                    let response = format!("HTTP/1.1 200 OK\r\n\r\n{}", rest);
                    stream.write_all(response.as_bytes()).await?;
                }
                _ => {
                    let response = format!("HTTP/1.1 404 NOT FOUND\r\n\r\n");
                    stream.write_all(response.as_bytes()).await?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}
