//! http_wasm — HTTP TCP Client + LED Blinky
//! Cible  : wasm32-unknown-unknown, no_std

#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", no_main)]

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }

// ================================================================
// PARAMÈTRES RÉSEAU — modifier avant compilation
// ================================================================
static WIFI_SSID:   &[u8] = b"a26nguep-hotspot";
static WIFI_PSK:    &[u8] = b"123456789";
static SERVER_IP:   &[u8] = b"10.42.0.1";
static DEVICE_NAME: &[u8] = b"heltec_v3";

const SERVER_PORT:     u32 = 8080;
const NETWORK_TIMEOUT: u32 = 30;
const SOCKET_TIMEOUT:  u32 = 5;
const SEND_INTERVAL:   u32 = 3;

// ================================================================
// HOST FUNCTIONS
// ================================================================
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_print(msg_ptr: *const u8, msg_len: u32);
    fn host_wifi_connect(
        ssid_ptr: *const u8, ssid_len: u32,
        psk_ptr:  *const u8, psk_len:  u32,
    ) -> i32;
    fn host_wait_network_ready(timeout_secs: u32) -> i32;
    fn host_gpio_blink();
    fn host_tcp_connect(
        ip_ptr: *const u8, ip_len: u32,
        port: u32, timeout_secs: u32,
    ) -> i32;
    fn host_tcp_send(fd: i32, buf_ptr: *const u8, buf_len: u32) -> i32;
    fn host_tcp_recv(fd: i32, buf_ptr: *mut u8,  buf_len: u32) -> i32;
    fn host_tcp_close(fd: i32);
    fn host_sleep(secs: u32);
}

// ================================================================
// BUFFERS STATIQUES
// ================================================================
static mut TX_BUF:   [u8; 512] = [0u8; 512];
static mut RX_BUF:   [u8; 512] = [0u8; 512];
static mut BODY_BUF: [u8; 128] = [0u8; 128];
static mut LOG_BUF:  [u8; 128] = [0u8; 128];

// ================================================================
// UTILITAIRES
// ================================================================

/// Copie src dans dst[offset..] et retourne le nouvel offset.
fn write_bytes(dst: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let end = offset + src.len();
    dst[offset..end].copy_from_slice(src);
    end
}

/// Écrit n en décimal ASCII dans dst[offset..] et retourne le nouvel offset.
fn write_u32(dst: &mut [u8], offset: usize, mut n: u32) -> usize {
    if n == 0 { dst[offset] = b'0'; return offset + 1; }
    let mut tmp = [0u8; 10];
    let mut len = 0usize;
    while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    for i in 0..len { dst[offset + i] = tmp[len - 1 - i]; }
    offset + len
}

// ================================================================
// LOG — affiche un message via host_print
// ================================================================
#[cfg(target_arch = "wasm32")]
fn log(msg: &[u8]) {
    unsafe { host_print(msg.as_ptr(), msg.len() as u32); }
}

/// Log avec un entier : "prefix" + N + "\n"
#[cfg(target_arch = "wasm32")]
fn log_num(prefix: &[u8], n: u32) {
    // Construire le message dans LOG_BUF via un slice mutable explicite
    let msg_len = unsafe {
        let buf: &mut [u8; 128] = &mut *(&raw mut LOG_BUF);
        let plen = prefix.len().min(100);
        buf[..plen].copy_from_slice(&prefix[..plen]);
        let mut i = plen;
        i = write_u32(buf, i, n);
        buf[i] = b'\n';
        i + 1
    };
    unsafe {
        let ptr = (&raw const LOG_BUF) as *const u8;
        host_print(ptr, msg_len as u32);
    }
}

// ================================================================
// send_http_post(seq)
// ================================================================
#[cfg(target_arch = "wasm32")]
fn send_http_post(seq: u32) {
    log_num(b"[HTTP] POST seq=", seq);

    // Construire corps JSON et requête HTTP dans des slices mutables
    let tx_len = unsafe {
        let body: &mut [u8; 128] = &mut *(&raw mut BODY_BUF);
        let mut i = 0;
        i = write_bytes(body, i, b"{\"device\":\"");
        i = write_bytes(body, i, DEVICE_NAME);
        i = write_bytes(body, i, b"\",\"seq\":");
        i = write_u32(body, i, seq);
        i = write_bytes(body, i, b",\"metric\":\"ping\",\"value\":1}");
        let body_len = i;

        let tx: &mut [u8; 512] = &mut *(&raw mut TX_BUF);
        let mut j = 0;
        j = write_bytes(tx, j, b"POST /data HTTP/1.0\r\nHost: ");
        j = write_bytes(tx, j, SERVER_IP);
        j = write_bytes(tx, j, b":");
        j = write_u32(tx, j, SERVER_PORT);
        j = write_bytes(tx, j,
            b"\r\nContent-Type: application/json\r\nContent-Length: ");
        j = write_u32(tx, j, body_len as u32);
        j = write_bytes(tx, j, b"\r\nConnection: close\r\n\r\n");
        // Copier le body dans tx (les deux buffers sont distincts)
        let body_slice: &[u8; 128] = &*(&raw const BODY_BUF);
        j = write_bytes(tx, j, &body_slice[..body_len]);
        j
    };

    // Connexion TCP
    let fd = unsafe {
        host_tcp_connect(
            SERVER_IP.as_ptr(), SERVER_IP.len() as u32,
            SERVER_PORT, SOCKET_TIMEOUT,
        )
    };
    if fd < 0 {
        log(b"[HTTP] TCP connect failed\n");
        return;
    }
    log(b"[HTTP] TCP connected\n");

    // Envoi
    let sent = unsafe {
        let tx_ptr = (&raw const TX_BUF) as *const u8;
        host_tcp_send(fd, tx_ptr, tx_len as u32)
    };

    if sent > 0 {
        log(b"[HTTP] request sent, waiting ACK...\n");
        let received = unsafe {
            let rx_ptr = (&raw mut RX_BUF) as *mut u8;
            host_tcp_recv(fd, rx_ptr, (512 - 1) as u32)
        };
        if received > 0 {
            log(b"[HTTP] ACK received\n");
        }
    } else {
        log(b"[HTTP] send failed\n");
    }

    unsafe { host_tcp_close(fd); }
    log(b"[HTTP] socket closed\n");
}

// ================================================================
// POINT D'ENTRÉE WASM
// ================================================================
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn main() {
    log(b"============================================\n");
    log(b" WASM HTTP TCP Client + Blinky\n");
    log(b"============================================\n");
    log(b"Connexion Wi-Fi...\n");

    let ret = unsafe {
        host_wifi_connect(
            WIFI_SSID.as_ptr(), WIFI_SSID.len() as u32,
            WIFI_PSK.as_ptr(),  WIFI_PSK.len()  as u32,
        )
    };
    if ret != 0 {
        log(b"[ERR] wifi_connect failed\n");
        return;
    }

    log(b"Attente IP DHCP...\n");
    let ret = unsafe { host_wait_network_ready(NETWORK_TIMEOUT) };
    if ret != 0 {
        log(b"[ERR] DHCP timeout\n");
        return;
    }
    log(b"Reseau pret\n");

    let mut seq: u32 = 0;
    loop {
        seq += 1;
        unsafe { host_gpio_blink(); }
        send_http_post(seq);
        unsafe { host_sleep(SEND_INTERVAL); }
    }
}

// ================================================================
// STUB x86_64 — pour rust-analyzer et cargo check
// ================================================================
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("Ce binaire est concu pour wasm32-unknown-unknown.");
    eprintln!("Compiler avec : bash build_wasm.sh");
}