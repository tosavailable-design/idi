use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};

async fn c(p: &str, a: &str) -> bool {
    let mut s = match TcpStream::connect(a).await {
        Ok(x) => x,
        Err(_) => return false,
    };
    match p {
        "http" => {
            if s.write_all(b"CONNECT 1.1.1.1:80 HTTP/1.1\r\nHost: 1.1.1.1:80\r\n\r\n").await.is_err() { return false; }
            let mut b = [0; 12];
            if s.read_exact(&mut b).await.is_err() { return false; }
            b.starts_with(b"HTTP/1.1 200") || b.starts_with(b"HTTP/1.0 200")
        }
        "socks4" => {
            if s.write_all(&[0x04, 0x01, 0x00, 0x50, 0x01, 0x01, 0x01, 0x01, 0x00]).await.is_err() { return false; }
            let mut b = [0; 8];
            if s.read_exact(&mut b).await.is_err() { return false; }
            b[0] == 0x00 && b[1] == 0x5A
        }
        "socks5" => {
            if s.write_all(&[0x05, 0x01, 0x00]).await.is_err() { return false; }
            let mut b1 = [0; 2];
            if s.read_exact(&mut b1).await.is_err() { return false; }
            if b1[0] != 0x05 || b1[1] != 0x00 { return false; }
            if s.write_all(&[0x05, 0x01, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x50]).await.is_err() { return false; }
            let mut b2 = [0; 10];
            if s.read_exact(&mut b2).await.is_err() { return false; }
            b2[0] == 0x05 && b2[1] == 0x00
        }
        _ => false,
    }
}

#[tokio::main]
async fn main() {
    let u = [
        ("http", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/http.txt"),
        ("socks4", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks4.txt"),
        ("socks5", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks5.txt"),
        ("http", "https://raw.githubusercontent.com/proxifly/free-proxy-list/refs/heads/main/proxies/protocols/http/data.txt"),
        ("http", "https://raw.githubusercontent.com/proxifly/free-proxy-list/refs/heads/main/proxies/protocols/https/data.txt"),
        ("socks4", "https://raw.githubusercontent.com/proxifly/free-proxy-list/refs/heads/main/proxies/protocols/socks4/data.txt"),
        ("socks5", "https://raw.githubusercontent.com/proxifly/free-proxy-list/refs/heads/main/proxies/protocols/socks5/data.txt"),
    ];
    let mut n = HashSet::new();
    let mut p = Vec::new();
    for (k, v) in u {
        if let Ok(o) = Command::new("curl").arg("-s").arg(v).output() {
            for l in String::from_utf8_lossy(&o.stdout).lines() {
                let l = l.trim();
                if !l.is_empty() && n.insert(l.to_string()) {
                    p.push((k, l.to_string()));
                }
            }
        }
    }
    let sm = Arc::new(Semaphore::new(10000));
    let r = Arc::new(Mutex::new(Vec::new()));
    let mut t = Vec::new();
    for (k, a) in p {
        let sm = sm.clone();
        let r = r.clone();
        let k = k.to_string();
        t.push(tokio::spawn(async move {
            let _p = sm.acquire().await.unwrap();
            if let Ok(true) = timeout(Duration::from_millis(2500), c(&k, &a)).await {
                r.lock().unwrap().push(format!("{}://{}", k, a));
            }
        }));
    }
    for x in t {
        let _ = x.await;
    }
    let f = r.lock().unwrap();
    print!("{}", f.join(","));
}
