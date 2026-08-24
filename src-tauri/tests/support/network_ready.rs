use std::time::{Duration, Instant};

const SERVER_READY_DEADLINE: Duration = Duration::from_secs(10);

/// Return a live TCP connection once the server is observably listening, or fail with the last
/// connection error at a liveness deadline. Callers may keep it open for a silent-client test or
/// drop it when they only need a readiness probe.
pub async fn connect_when_ready(port: u16) -> tokio::net::TcpStream {
    let address = format!("127.0.0.1:{port}");
    let start = Instant::now();

    loop {
        match tokio::net::TcpStream::connect(&address).await {
            Ok(stream) => return stream,
            Err(error) => {
                let waited = start.elapsed();
                assert!(
                    waited < SERVER_READY_DEADLINE,
                    "server {address} was not reachable after {waited:.2?}; last error: {error}"
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
