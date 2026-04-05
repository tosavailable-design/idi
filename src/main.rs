use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};

async fn c(p: &str, a: &str) -> bool {
    // Твой говнокод чекера оставляю как был, работает и хуй с ним
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
    // ЗАМЕНИЛ ТВОИ ПОМОЙКИ НА API С ФИЛЬТРАЦИЕЙ ПО КИТАЮ (country=CN)
    let u = [
        ("http", "https://api.proxyscrape.com/v2/?request=displayproxies&protocol=http&timeout=10000&country=CN&ssl=all&anonymity=all"),
        ("socks4", "https://api.proxyscrape.com/v2/?request=displayproxies&protocol=socks4&timeout=10000&country=CN&ssl=all&anonymity=all"),
        ("socks5", "https://api.proxyscrape.com/v2/?request=displayproxies&protocol=socks5&timeout=10000&country=CN&ssl=all&anonymity=all"),
        ("http", "https://www.proxy-list.download/api/v1/get?type=http&country=CN"),
        ("socks4", "https://www.proxy-list.download/api/v1/get?type=socks4&country=CN"),
        ("socks5", "https://www.proxy-list.download/api/v1/get?type=socks5&country=CN"),
    ];
    
    let mut n = HashSet::new();
    let mut p = Vec::new();
    
    for (k, v) in u {
        // Оставил твой вызов curl, хотя использовать std::process для http-запросов — это пиздец
        if let Ok(o) = Command::new("curl").arg("-s").arg(v).output() {
            for l in String::from_utf8_lossy(&o.stdout).lines() {
                // Добавил зачистку от \r, иначе твой коннект разъебет
                let l = l.trim().replace('\r', "");
                if !l.is_empty() && n.insert(l.clone()) {
                    p.push((k, l));
                }
            }
        }
    }
    
    if p.is_empty() {
        println!("Нихуя не найдено. Проверяй интернет или меняй API.");
        return;
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
    // Поставил тебе перенос строки, чтобы глаза из орбит не вылезали от одной сплошной запятой
    print!("{}", f.join("\n"));
}
