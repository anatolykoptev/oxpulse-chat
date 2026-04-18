//! Minimal STUN Binding-Request health probe.
//!
//! This is the lowest primitive the TURN probe loop needs: send one
//! 20-byte STUN Binding-Request over UDP to a [`SocketAddr`] and measure
//! how long it takes to get a Binding-Response back whose transaction ID
//! matches. No attribute parsing, no auth, no history — just proof of life.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use getrandom::getrandom as getrandom_fill;
use tokio::net::UdpSocket;
use tokio::time::timeout as tokio_timeout;

/// STUN magic cookie (RFC 5389 §6). Present in every STUN message.
const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;

/// Binding-Request message type (RFC 5389 §3). Class = Request, Method = Binding.
const STUN_BINDING_REQUEST: u16 = 0x0001;

/// Binding-Response classes (RFC 5389 §6). We accept either success or error as
/// proof of life: the server is reachable and talking STUN back at us. Health
/// means "the server exists and responds", not "the server approves of our auth".
const STUN_BINDING_SUCCESS: u16 = 0x0101;
const STUN_BINDING_ERROR: u16 = 0x0111;

/// Sends a STUN Binding-Request to `addr` over UDP and returns the RTT
/// in milliseconds, or an error string if the probe fails or times out.
///
/// The probe is intentionally minimal: we only care whether the server
/// responds with a well-formed Binding-Response whose transaction ID
/// matches our request. We do not parse attributes, verify the MAPPED-
/// ADDRESS, or authenticate. For TURN servers that require auth, the
/// STUN Binding-Request is allowed unauthenticated (RFC 5389).
pub async fn probe(addr: SocketAddr, timeout: Duration) -> Result<u32, String> {
    // Bind an ephemeral local UDP socket. Matching address family to the target
    // avoids "address family mismatch" errors when probing an IPv6 relay from
    // a v4-only binding.
    let bind_addr: SocketAddr = match addr {
        SocketAddr::V4(_) => "0.0.0.0:0".parse().unwrap(),
        SocketAddr::V6(_) => "[::]:0".parse().unwrap(),
    };
    let sock = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("bind: {e}"))?;
    sock.connect(addr)
        .await
        .map_err(|e| format!("connect: {e}"))?;

    // Build the 20-byte STUN header with a random 96-bit transaction ID.
    // No attributes (zero-length body).
    let mut req = [0u8; 20];
    req[0..2].copy_from_slice(&STUN_BINDING_REQUEST.to_be_bytes());
    req[2..4].copy_from_slice(&0u16.to_be_bytes());
    req[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    // Fill 12-byte transaction ID with random bytes.
    let mut tid = [0u8; 12];
    getrandom_fill(&mut tid).map_err(|e| format!("rand: {e}"))?;
    req[8..20].copy_from_slice(&tid);

    let started = Instant::now();
    sock.send(&req).await.map_err(|e| format!("send: {e}"))?;

    // Read until we see a response matching our TID, or timeout.
    // We loop because a stray packet on an ephemeral socket could in theory
    // arrive first (defensive — UdpSocket::connect filters by default, but
    // some stacks don't honor that strictly).
    let mut buf = [0u8; 1500];
    let read_matching_response = async {
        loop {
            let n = sock
                .recv(&mut buf)
                .await
                .map_err(|e| format!("recv: {e}"))?;
            if n < 20 {
                continue; // too short to be a STUN message
            }
            let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
            if msg_type != STUN_BINDING_SUCCESS && msg_type != STUN_BINDING_ERROR {
                continue; // not a Binding-Response class we care about
            }
            let cookie = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
            if cookie != STUN_MAGIC_COOKIE {
                continue;
            }
            if buf[8..20] != tid {
                continue; // stale/foreign response
            }
            return Ok::<(), String>(());
        }
    };

    tokio_timeout(timeout, read_matching_response)
        .await
        .map_err(|_| format!("timeout after {:?}", timeout))??;

    Ok(started.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_returns_rtt_on_success_response() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let (n, peer) = server.recv_from(&mut buf).await.unwrap();
            // Build a Binding-Success response with the same TID.
            let mut resp = [0u8; 20];
            resp[0..2].copy_from_slice(&STUN_BINDING_SUCCESS.to_be_bytes());
            resp[2..4].copy_from_slice(&0u16.to_be_bytes());
            resp[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
            resp[8..20].copy_from_slice(&buf[8..20]); // echo TID
            server.send_to(&resp, peer).await.unwrap();
            let _ = n;
        });

        let rtt = probe(addr, Duration::from_secs(1)).await.unwrap();
        assert!(
            rtt < 1000,
            "RTT {}ms should be sub-second on localhost",
            rtt
        );
    }

    #[tokio::test]
    async fn probe_times_out_on_silent_server() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        // Hold the socket open but never send a reply.
        let _guard = server;
        let err = probe(addr, Duration::from_millis(100))
            .await
            .expect_err("probe should time out");
        assert!(
            err.contains("timeout"),
            "error should mention timeout, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn probe_ignores_responses_with_wrong_transaction_id() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let (_, peer) = server.recv_from(&mut buf).await.unwrap();

            // First: wrong TID (all zeros — statistically different from the
            // client's random 96-bit TID).
            let mut bad = [0u8; 20];
            bad[0..2].copy_from_slice(&STUN_BINDING_SUCCESS.to_be_bytes());
            bad[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
            server.send_to(&bad, peer).await.unwrap();

            // Second: correct TID.
            let mut good = [0u8; 20];
            good[0..2].copy_from_slice(&STUN_BINDING_SUCCESS.to_be_bytes());
            good[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
            good[8..20].copy_from_slice(&buf[8..20]);
            server.send_to(&good, peer).await.unwrap();
        });

        let rtt = probe(addr, Duration::from_secs(1)).await.unwrap();
        assert!(rtt < 1000);
    }
}
