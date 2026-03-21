use std::env;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use futures::stream::StreamExt;

#[derive(Clone, Copy)]
enum PType { Http, Socks4, Socks5 }

async fn check(addr: String, t: PType) -> Option<String> {
    let mut s = TcpStream::connect(&addr).await.ok()?;
    match t {
        PType::Http => {
            s.write_all(b"CONNECT 1.1.1.1:80 HTTP/1.1\r\nHost: 1.1.1.1:80\r\n\r\n").await.ok()?;
            let mut buf = [0; 12];
            s.read_exact(&mut buf).await.ok()?;
            if !buf.starts_with(b"HTTP/1.") || &buf[9..12] != b"200" { return None; }
        }
        PType::Socks4 => {
            s.write_all(&[4, 1, 0, 80, 1, 1, 1, 1, 0]).await.ok()?;
            let mut buf = [0; 2];
            s.read_exact(&mut buf).await.ok()?;
            if buf[1] != 0x5a { return None; }
        }
        PType::Socks5 => {
            s.write_all(&[5, 1, 0]).await.ok()?;
            let mut buf = [0; 2];
            s.read_exact(&mut buf).await.ok()?;
            if buf[1] != 0 { return None; }
            s.write_all(&[5, 1, 0, 1, 1, 1, 1, 1, 0, 80]).await.ok()?;
            let mut buf = [0; 2];
            s.read_exact(&mut buf).await.ok()?;
            if buf[1] != 0 { return None; }
        }
    }
    Some(addr)
}

#[tokio::main]
async fn main() {
    let urls = [
        ("https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/http.txt", PType::Http),
        ("https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks4.txt", PType::Socks4),
        ("https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks5.txt", PType::Socks5),
    ];

    let mut proxies = Vec::new();
    for (url, t) in urls {
        if let Ok(resp) = reqwest::get(url).await {
            if let Ok(text) = resp.text().await {
                for line in text.lines() {
                    let addr = line.trim().to_string();
                    if !addr.is_empty() {
                        proxies.push((addr, t));
                    }
                }
            }
        }
    }

    let valid: Vec<String> = futures::stream::iter(proxies)
        .map(|(addr, t)| {
            tokio::spawn(async move {
                tokio::time::timeout(Duration::from_secs(1), check(addr, t)).await.ok().flatten()
            })
        })
        .buffer_unordered(500)
        .filter_map(|res| std::future::ready(res.unwrap_or(None)))
        .collect().await;

    let result = valid.join(",");
    env::set_var("PROXY_LIST", &result);
    println!("{}", result);
}
