use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};

async fn c(p: &str, a: &str, os_errors: Arc<AtomicUsize>, tcp_ok: Arc<AtomicUsize>) -> bool {
    let mut s = match TcpStream::connect(a).await {
        Ok(x) => {
            tcp_ok.fetch_add(1, Ordering::SeqCst); // Считаем успешные TCP коннекты
            x
        },
        Err(e) => {
            // Проверяем, не послала ли нас ОС из-за лимита сокетов
            if let Some(os_err) = e.raw_os_error() {
                // 24 = Too many open files (Linux/Mac)
                // 10055 = WSAENOBUFS (Windows)
                // 10048 = WSAEADDRINUSE (Windows)
                if os_err == 24 || os_err == 10055 || os_err == 10048 {
                    os_errors.fetch_add(1, Ordering::SeqCst); // Считаем отказы ОС
                }
            }
            return false;
        }
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
    
    let total_loaded = p.len();
    println!("Загружено уникальных прокси: {}", total_loaded);

    let sm = Arc::new(Semaphore::new(10000));
    let r = Arc::new(Mutex::new(Vec::new()));
    
    // СЧЕТЧИКИ ДЛЯ ДОКАЗАТЕЛЬСТВА
    let os_errors = Arc::new(AtomicUsize::new(0));
    let tcp_ok = Arc::new(AtomicUsize::new(0));
    let timeout_errors = Arc::new(AtomicUsize::new(0));

    let mut t = Vec::new();
    for (k, a) in p {
        let sm = sm.clone();
        let r = r.clone();
        let k = k.to_string();
        
        let os_err_clone = os_errors.clone();
        let tcp_ok_clone = tcp_ok.clone();
        let timeout_err_clone = timeout_errors.clone();

        t.push(tokio::spawn(async move {
            let _p = sm.acquire().await.unwrap();
            
            match timeout(Duration::from_millis(2500), c(&k, &a, os_err_clone, tcp_ok_clone)).await {
                Ok(true) => r.lock().unwrap().push(format!("{}://{}", k, a)),
                Ok(false) => {}, // Прокси мертв или вернул мусор
                Err(_) => {
                    timeout_err_clone.fetch_add(1, Ordering::SeqCst); // Не успели за 2.5 сек
                }
            }
        }));
    }
    
    for x in t {
        let _ = x.await;
    }
    
    let f = r.lock().unwrap();
    
    println!("\n=== ДОКАЗАТЕЛЬСТВО ===");
    println!("Всего попыток: {}", total_loaded);
    println!("Успешных TCP коннектов (дошли до интернета): {}", tcp_ok.load(Ordering::SeqCst));
    println!("ОТКАЗЫ ОПЕРАЦИОННОЙ СИСТЕМЫ (Нехватка портов/сокетов): {}", os_errors.load(Ordering::SeqCst));
    println!("Отвалились по таймауту (2.5 сек): {}", timeout_errors.load(Ordering::SeqCst));
    println!("Найдено рабочих прокси: {}", f.len());
    println!("======================\n");
    
    // print!("{}", f.join(",")); // Закомментировал вывод списка, чтобы было видно логи
}
