//! http_wasm
//!
//! Reproduit exactement la logique de main.c Zephyr :
//!   1. Blink LED (simulé via println! / host import sur WAMR embarqué)
//!   2. Connexion TCP au serveur
//!   3. Envoi HTTP POST JSON (device, seq, metric, value)
//!   4. Lecture ACK serveur
//!   5. Attente 3 secondes
//!   6. Boucle infinie
//!
//! Compilation :
//!   rustc --target wasm32-wasip1 src/main.rs -o http_client_rust.wasm
//!   (ou : cargo build --target wasm32-wasip1 --release)
//!
//! Correspondance Zephyr ↔ Rust/WASI :
//!   zsock_socket()     ↔  TcpStream::connect()
//!   zsock_send()       ↔  stream.write_all()
//!   zsock_recv()       ↔  stream.read()
//!   zsock_close()      ↔  drop(stream) [automatique]
//!   k_sleep(3s)        ↔  thread::sleep(Duration::from_secs(3))
//!   LOG_INF()          ↔  println!() / eprintln!()

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

// ==================================================================
// PARAMÈTRES — identiques à main.c Zephyr
// ==================================================================
const SERVER_IP:   &str = "10.42.0.1";
const SERVER_PORT: u16  = 8080;
const DEVICE_NAME: &str = "heltec_v3";

// ==================================================================
// blink_led() — équivalent Zephyr GPIO blinky
//
// En WASI pur : println!
// En WAMR embarqué Zephyr : appeler une fonction host native déclarée
// avec #[link(wasm_import_module = "env")] extern "C" { fn host_gpio_blink(); }
// ==================================================================
fn blink_led() {
    println!("[LED] ** BLINK **");
}

// ==================================================================
// send_http_post() — logique identique à Zephyr
//
// Rust std::net::TcpStream encapsule le flux TCP BSD socket :
//   TcpStream::connect()  →  socket() + connect()
//   stream.write_all()    →  zsock_send()
//   stream.read()         →  zsock_recv()
//   drop(stream)          →  zsock_close() [automatique en fin de scope]
// ==================================================================
fn send_http_post(seq: u32) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("{}:{}", SERVER_IP, SERVER_PORT);

    // 1+4. Créer socket TCP et connecter (= zsock_socket + zsock_connect)
    println!("[{}] Connexion TCP → {}...", seq, addr);
    let mut stream = TcpStream::connect(&addr)?;
    println!("[{}] Connexion TCP établie", seq);

    // 2. Timeout 5s (= zsock_setsockopt SO_RCVTIMEO/SO_SNDTIMEO)
    let timeout = Duration::from_secs(5);
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    // 5. Corps JSON — identique à Zephyr
    let body = format!(
        "{{\"device\":\"{}\",\"seq\":{},\"metric\":\"ping\",\"value\":1}}",
        DEVICE_NAME, seq
    );

    // 6. Requête HTTP POST — identique à Zephyr
    let request = format!(
        "POST /data HTTP/1.0\r\n\
         Host: {}:{}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        SERVER_IP, SERVER_PORT,
        body.len(),
        body
    );

    // 7. Envoyer (= zsock_send)
    println!("[{}] Envoi HTTP POST ({} octets)...", seq, request.len());
    stream.write_all(request.as_bytes())?;
    println!("[{}] {} octets envoyés", seq, request.len());

    // 8. Lire réponse (= zsock_recv)
    let mut rx_buf = [0u8; 512];
    let n = stream.read(&mut rx_buf)?;

    if n == 0 {
        eprintln!("[{}] Connexion fermée sans réponse", seq);
        return Err("no response".into());
    }

    let response = String::from_utf8_lossy(&rx_buf[..n]);

    // Extraire corps (après \r\n\r\n) — identique à Zephyr strstr
    if let Some(pos) = response.find("\r\n\r\n") {
        println!("[{}] *** ACK SERVEUR : {} ***", seq, &response[pos + 4..]);
    } else {
        println!("[{}] Réponse brute : {}", seq, response);
    }

    // 9. Fermer socket — automatique (drop de TcpStream)
    println!("[{}] Socket TCP fermé", seq);
    Ok(())
}

// ==================================================================
// main() — boucle identique à Zephyr
// ==================================================================
fn main() {
    println!("============================================");
    println!(" WASM HTTP TCP Client + Blinky  [Rust v1]");
    println!(" Device  : {}", DEVICE_NAME);
    println!(" Serveur : {}:{}", SERVER_IP, SERVER_PORT);
    println!("============================================");

    let mut seq: u32 = 0;

    loop {
        seq += 1;

        // Blink (simulé — ou host import sur WAMR)
        blink_led();

        // HTTP POST
        match send_http_post(seq) {
            Ok(()) => {}
            Err(e) => eprintln!("Envoi [{}] échoué ({:?}), retry dans 3s", seq, e),
        }

        // Attendre 3 secondes (= k_sleep(K_SECONDS(3)))
        thread::sleep(Duration::from_secs(3));
    }
}