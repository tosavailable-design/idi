use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};

// Структура для подсетей
struct Cidr {
    base: u32,
    mask: u32,
}

// Превращаем строковый IP в 32-битное число, хули
fn ip_to_u32(ip: &str) -> Option<u32> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut res = 0u32;
    for (i, p) in parts.iter().enumerate() {
        let octet: u32 = p.parse().ok()?;
        res |= octet << (24 - i * 8);
    }
    Some(res)
}

// Парсим формат типа 1.1.1.0/24 в базовый IP и маску
fn parse_cidr(cidr: &str) -> Option<Cidr> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.is_empty() { return None; }
    
    let base = ip_to_u32(parts[0])?;
    let prefix: u32 = if parts.len() == 2 {
        parts[1].parse().ok()?
    } else {
        32 // Если маски нет, считаем как единичный IP
    };

    let mask = if prefix == 0 {
        0
    } else {
        (!0u32) << (32 - prefix)
    };

    Some(Cidr {
        base: base & mask,
        mask,
    })
}

// Твой ебучий чекер коннектов
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
    // 1. Качаем сырые диапазоны китайских IP (чтобы фильтровать твои говносписки)
    eprintln!(">>> Качаем список китайских IP-диапазонов...");
    let mut cn_cidrs = Vec::new();
    if let Ok(o) = Command::new("curl").arg("-s").arg("https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt").output() {
        for l in String::from_utf8_lossy(&o.stdout).lines() {
            let l = l.trim();
            if l.is_empty() { continue; }
            if let Some(cidr) = parse_cidr(l) {
                cn_cidrs.push(cidr);
            }
        }
    }
    
    if cn_cidrs.is_empty() {
        eprintln!("Пиздец, не удалось скачать базу Китая. Проверь инет.");
        return;
    }
    eprintln!(">>> Загружено {} китайских подсетей. Начинаем парсить свалку...", cn_cidrs.len());

    // Твои ненаглядные старые ссылки на помойки
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
                // Чистим от виндовской каретки, сука, не убирай это!
                let l = l.trim().replace('\r', "");
                if l.is_empty() { continue; }
                
                // Вытаскиваем только IP (всё, что до двоеточия)
                let ip_str = l.split(':').next().unwrap_or("");
                
                if let Some(ip_u32) = ip_to_u32(ip_str) {
                    // Проверяем, в Китае ли этот говно-айпи
                    let mut is_cn = false;
                    for cidr in &cn_cidrs {
                        if (ip_u32 & cidr.mask) == cidr.base {
                            is_cn = true;
                            break;
                        }
                    }
                    
                    // Если Китай и еще не было в сете — добавляем на проверку
                    if is_cn && n.insert(l.clone()) {
                        p.push((k, l));
                    }
                }
            }
        }
    }

    eprintln!(">>> Найдено {} уникальных китайских прокси. Запускаем чек...", p.len());

    let sm = Arc::new(Semaphore::new(10000));
    let r = Arc::new(Mutex::new(Vec::new()));
    let mut t = Vec::new();
    
    for (k, a) in p {
        let sm = sm.clone();
        let r = r.clone();
        let k = k.to_string();
        t.push(tokio::spawn(async move {
            let _p = sm.acquire().await.unwrap();
            // Таймаут оставил твой, можешь увеличить, если китаезы тупят
            if let Ok(true) = timeout(Duration::from_millis(2500), c(&k, &a)).await {
                r.lock().unwrap().push(format!("{}://{}", k, a));
            }
        }));
    }
    
    for x in t {
        let _ = x.await;
    }
    
    let f = r.lock().unwrap();
    eprintln!(">>> ГОТОВО! Валидные прокси:");
    print!("{}", f.join("\n"));
}
