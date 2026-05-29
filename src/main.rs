//! http_wasm — HTTP TCP Client + LED Blinky (Rust WASI)
//!
//! Correspondance Zephyr ↔ Rust/WASI :
//!   WIFI_SSID / WIFI_PSK              ↔  WIFI_SSID / WIFI_PSK (const)
//!   net_mgmt(WIFI_CONNECT)            ↔  host_wifi_connect(ssid, psk)
//!   on_wifi_event / on_dhcp_event     ↔  géré côté WAMR/Zephyr
//!   k_sem_take(&net_ready, 30s)       ↔  host_wait_network_ready(30)
//!   zsock_socket() + connect()        ↔  TcpStream::connect()
//!   zsock_setsockopt(SO_RCVTIMEO)     ↔  stream.set_read_timeout()
//!   zsock_setsockopt(SO_SNDTIMEO)     ↔  stream.set_write_timeout()
//!   zsock_send()                      ↔  stream.write_all() + flush()
//!   zsock_recv()                      ↔  stream.read()
//!   zsock_close()                     ↔  drop(stream)
//!   k_sleep(K_SECONDS(3))             ↔  thread::sleep(Duration::from_secs(3))
//!   LOG_INF()                         ↔  println!()
//!   LOG_ERR() / LOG_WRN()             ↔  eprintln!()
//!   GPIO blinky                       ↔  host_gpio_blink()

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

// ==================================================================
// PARAMÈTRES RÉSEAU
// ==================================================================
const WIFI_SSID: &str = "a26nguep-hotspot";
const WIFI_PSK:  &str = "123456789";

// ==================================================================
// PARAMÈTRES SERVEUR
// ==================================================================
const SERVER_IP:   &str = "10.42.0.1";
const SERVER_PORT: u16  = 8080;
const DEVICE_NAME: &str = "heltec_v3";

const NETWORK_TIMEOUT_SECS: u32 = 30;
const SOCKET_TIMEOUT_SECS:  u64 = 5;
const SEND_INTERVAL_SECS:   u64 = 3;

// ==================================================================
// HOST FUNCTIONS — importées depuis WAMR/Zephyr
//
// Ces fonctions sont fournies par le runtime WAMR embarqué dans
// Zephyr. WAMR les injecte lors de l'instantiation du module WASM.
// Elles sont enregistrées dans main.c Zephyr via
// wasm_runtime_register_natives("env", native_symbols, ...).
//
//   host_wifi_connect()       ↔  net_mgmt(NET_REQUEST_WIFI_CONNECT)
//   host_wait_network_ready() ↔  k_sem_take(&net_ready_wamr, ...)
//   host_gpio_blink()         ↔  gpio_pin_set_dt() + k_msleep(150)
// ==================================================================
#[link(wasm_import_module = "env")]
extern "C" {
    /// Déclenche la connexion Wi-Fi côté Zephyr.
    /// Retourne 0 si la demande est acceptée, < 0 en cas d'erreur.
    fn host_wifi_connect(
        ssid_ptr: *const u8, ssid_len: u32,
        psk_ptr:  *const u8, psk_len:  u32,
    ) -> i32;

    /// Bloque jusqu'à l'obtention de l'IP DHCP ou expiration du timeout.
    /// Retourne 0 si le réseau est prêt, -1 si timeout.
    fn host_wait_network_ready(timeout_secs: u32) -> i32;

    /// Fait clignoter la LED GPIO via Zephyr (150 ms ON, 150 ms OFF).
    fn host_gpio_blink();
}

// ==================================================================
// wifi_connect_init()
//
// Équivalent de wifi_connect() dans main.c Zephyr.
// Envoie les credentials Wi-Fi au runtime Zephyr via host functions,
// puis attend l'attribution de l'IP DHCP (timeout 30 s).
// ==================================================================
fn wifi_connect_init() -> Result<(), String> {
    println!("============================================");
    println!(" Configuration reseau Wi-Fi");
    println!(" SSID : {}", WIFI_SSID);
    println!("============================================");

    println!("Connexion Wi-Fi -> SSID : \"{}\"", WIFI_SSID);

    // ↔ net_mgmt(NET_REQUEST_WIFI_CONNECT, iface, &params, sizeof(params))
    let ret = unsafe {
        host_wifi_connect(
            WIFI_SSID.as_ptr(), WIFI_SSID.len() as u32,
            WIFI_PSK.as_ptr(),  WIFI_PSK.len()  as u32,
        )
    };
    if ret != 0 {
        return Err(format!("Echec WIFI_CONNECT : code {}", ret));
    }

    // ↔ k_sem_take(&net_ready, K_SECONDS(30))
    println!("Attente IP DHCP (max {}s)...", NETWORK_TIMEOUT_SECS);
    let ret = unsafe { host_wait_network_ready(NETWORK_TIMEOUT_SECS) };
    if ret != 0 {
        return Err(format!(
            "Timeout : pas d'IP DHCP apres {}s. Verifier hotspot \"{}\" actif en 2,4 GHz.",
            NETWORK_TIMEOUT_SECS, WIFI_SSID
        ));
    }

    println!("Reseau pret — IP DHCP obtenue");
    println!("Serveur cible : {}:{}", SERVER_IP, SERVER_PORT);
    Ok(())
}

// ==================================================================
// blink_led()
//
// Équivalent de blink_led() dans main.c Zephyr.
// Appelle host_gpio_blink() qui déclenche le clignotement physique
// de la LED onboard via les GPIO Zephyr.
// ==================================================================
fn blink_led() {
    // ↔ gpio_pin_set_dt(&led, 1) + k_msleep(150) + gpio_pin_set_dt(&led, 0)
    unsafe { host_gpio_blink() };
}

// ==================================================================
// send_http_post()
//
// Équivalent de send_http_post() dans main.c Zephyr.
//
// Flux TCP identique à l'application native :
//   TcpStream::connect()        ↔  zsock_socket() + zsock_connect()
//   set_read_timeout(5s)        ↔  zsock_setsockopt(SO_RCVTIMEO, 5s)
//   set_write_timeout(5s)       ↔  zsock_setsockopt(SO_SNDTIMEO, 5s)
//   stream.write_all() + flush()↔  zsock_send()
//   stream.read()               ↔  zsock_recv()
//   drop(stream)                ↔  zsock_close()
// ==================================================================
fn send_http_post(seq: u32) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("{}:{}", SERVER_IP, SERVER_PORT);

    // ↔ zsock_socket() + zsock_connect()
    println!("[{}] Connexion TCP -> {} ...", seq, addr);
    let mut stream = TcpStream::connect(&addr).map_err(|e| {
        format!("zsock_connect failed : {}", e)
    })?;
    println!("[{}] Connexion TCP etablie", seq);

    // ↔ zsock_setsockopt SO_RCVTIMEO / SO_SNDTIMEO
    let timeout = Duration::from_secs(SOCKET_TIMEOUT_SECS);
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    // ↔ snprintf(body, ...)
    let body = format!(
        "{{\"device\":\"{}\",\"seq\":{},\"metric\":\"ping\",\"value\":1}}",
        DEVICE_NAME, seq
    );

    // ↔ snprintf(tx_buf, "POST /data HTTP/1.0\r\n...")
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

    // ↔ zsock_send()
    // flush() explicite pour garantir l'envoi complet avant le recv
    println!("[{}] Envoi HTTP POST ({} octets)...", seq, request.len());
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    println!("[{}] {} octets envoyes", seq, request.len());

    // ↔ zsock_recv()
    let mut rx_buf = [0u8; 512];
    let n = stream.read(&mut rx_buf)?;

    if n == 0 {
        eprintln!("[{}] Connexion fermee sans reponse", seq);
        return Err("no response".into());
    }

    let response = String::from_utf8_lossy(&rx_buf[..n]);

    // ↔ strstr(rx_buf, "\r\n\r\n")
    if let Some(pos) = response.find("\r\n\r\n") {
        println!("[{}] *** ACK SERVEUR : {} ***",
                 seq, response[pos + 4..].trim());
    } else {
        println!("[{}] Reponse brute : {}", seq, response.trim());
    }

    // ↔ zsock_close() — automatique via drop(stream)
    println!("[{}] Socket TCP ferme", seq);
    Ok(())
}

// ==================================================================
// main()
//
// Équivalent de main() dans main.c Zephyr.
// Séquence identique :
//   1. Bannière
//   2. Connexion Wi-Fi + DHCP
//   3. Boucle infinie : blink → POST → sleep(3s)
// ==================================================================
fn main() {
    println!("============================================");
    println!(" WASM HTTP TCP Client + Blinky  [Rust v2]");
    println!(" SSID    : {}", WIFI_SSID);
    println!(" Serveur : {}:{}", SERVER_IP, SERVER_PORT);
    println!("============================================");

    // ↔ int ret = wifi_connect(); if (ret != 0) { LOG_ERR; return ret; }
    if let Err(e) = wifi_connect_init() {
        eprintln!("Connexion reseau echouee : {}", e);
        std::process::exit(1);
    }

    println!(
        "Reseau pret — HTTP POST toutes les {}s vers {}:{}",
        SEND_INTERVAL_SECS, SERVER_IP, SERVER_PORT
    );

    // ↔ while (1) { seq++; blink_led(); send_http_post(seq); k_sleep(3s); }
    let mut seq: u32 = 0;
    loop {
        seq += 1;

        // ↔ blink_led()
        blink_led();

        // ↔ ret = send_http_post(seq);
        if let Err(e) = send_http_post(seq) {
            // ↔ LOG_WRN("Envoi [%d] echoue, retry dans 3s", seq, ret)
            eprintln!(
                "Envoi [{}] echoue ({:?}), retry dans {}s",
                seq, e, SEND_INTERVAL_SECS
            );
        }

        // ↔ k_sleep(K_SECONDS(3))
        thread::sleep(Duration::from_secs(SEND_INTERVAL_SECS));
    }
}