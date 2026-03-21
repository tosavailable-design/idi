use futures::stream::{self, StreamExt};
use reqwest::{Client, Proxy};
use std::collections::HashSet;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let urls = [
        ("http", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/http.txt"),
        ("socks4", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks4.txt"),
        ("socks5", "https://raw.githubusercontent.com/TheSpeedX/PROXY-List/refs/heads/master/socks5.txt"),
    ];

    let mut unique_proxies = HashSet::new();

    for (proto, url) in urls {
        if let Ok(resp) = reqwest::get(url).await {
            if let Ok(text) = resp.text().await {
                for line in text.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        unique_proxies.insert(format!("{}://{}", proto, line));
                    }
                }
            }
        }
    }

    let proxies: Vec<String> = unique_proxies.into_iter().collect();

    let working_proxies: Vec<String> = stream::iter(proxies)
        .map(|proxy_url| {
            tokio::spawn(async move {
                let proxy_obj = match Proxy::all(&proxy_url) {
                    Ok(p) => p,
                    Err(_) => return None,
                };
                
                let client = match Client::builder()
                    .proxy(proxy_obj)
                    .timeout(Duration::from_millis(2500))
                    .build() {
                        Ok(c) => c,
                        Err(_) => return None,
                };

                match client.get("http://1.1.1.1").send().await {
                    Ok(resp) if resp.status().is_success() => Some(proxy_url),
                    _ => None,
                }
            })
        })
        .buffer_unordered(10000)
        .filter_map(|res| async { res.unwrap_or(None) })
        .collect()
        .await;

    print!("{}", working_proxies.join(","));
}
