//! rx — nodo RECEPTOR LoRa (ESP32-C3 SuperMini + LR1121). PASO 3.  [NODO 3]
//!
//! Escucha en RX continuo los paquetes que manda el nodo TX y loguea por serial
//! seq / RSSI / SNR y el PER acumulado, para pruebas de distancia. Se lee con el
//! monitor serial (tools/serial-monitor). Todavía SIN ACK downlink ni BLE: primero
//! medir el uplink. El ACK (encode_ack + transmit_ack_and_return_rx) es el
//! siguiente escalón.
//!
//! Target C3: riscv32imc-esp-espidf · MCU=esp32c3 · consola USB-Serial/JTAG ·
//! /dev/ttyACM0.  Pinout LR1121 → ESP32-C3 (igual que el nodo 1):
//!   CS=7  CLK=4  MOSI=6  MISO=5  RESET=3  BUSY=10
//!
//! La radio arranca con los DEFAULT_* del driver: deben COINCIDIR con el TX
//! (916.8 MHz · SF7 · BW125 · CR4/5) o no engancha un solo paquete.

use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config as SpiConfig, SpiDeviceDriver, SpiDriver, SPI2},
    units::Hertz,
};
use esp_idf_svc::log::EspLogger;
use log::{error, info, warn};
use lr1121::{Lr1121, RxPacket, DEFAULT_BW_KHZ, DEFAULT_CR, DEFAULT_FREQ_HZ, DEFAULT_SF};

/// Extrae el seq u16 de un paquete `[b'P', b'2', seq_lo, seq_hi]`.
/// Devuelve None si no tiene el prefijo/longitud esperados.
fn parse_seq(payload: &[u8]) -> Option<u16> {
    if payload.len() >= 4 && payload[0] == b'P' && payload[1] == b'2' {
        Some(u16::from_le_bytes([payload[2], payload[3]]))
    } else {
        None
    }
}

/// Estadística acumulada del enlace (uplink).
struct LinkStats {
    received: u32,
    first_seq: Option<u16>,
    last_seq: u16,
    bad_crc: u32,
    unknown: u32,
}

impl LinkStats {
    fn new() -> Self {
        Self { received: 0, first_seq: None, last_seq: 0, bad_crc: 0, unknown: 0 }
    }

    /// Paquetes esperados en la ventana [first_seq .. last_seq] (asume que el TX
    /// manda seq monótono creciente; sin manejar wrap de u16 —una prueba de banco
    /// no llega a 65535 paquetes—).
    fn expected(&self) -> u32 {
        match self.first_seq {
            Some(f) => self.last_seq.wrapping_sub(f) as u32 + 1,
            None => 0,
        }
    }

    fn lost(&self) -> u32 {
        self.expected().saturating_sub(self.received)
    }

    /// PER en por mil (‰) para no usar float en el log.
    fn per_permille(&self) -> u32 {
        let exp = self.expected();
        if exp == 0 { 0 } else { self.lost() * 1000 / exp }
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("rx: paso 3 — nodo RECEPTOR (RX continuo + log de RSSI/SNR/PER)");

    let p = Peripherals::take().unwrap();

    // ─── SPI2: CLK=GPIO4  MOSI=GPIO6  MISO=GPIO5 ──────────────────────────────
    let spi_driver = SpiDriver::new::<SPI2>(
        p.spi2,
        p.pins.gpio4,
        p.pins.gpio6,
        Some(p.pins.gpio5),
        &esp_idf_hal::spi::SpiDriverConfig::new(),
    )
    .expect("SPI driver");

    let spi = SpiDeviceDriver::new(
        spi_driver,
        Some(p.pins.gpio7), // CS
        &SpiConfig::new().baudrate(Hertz(8_000_000)),
    )
    .expect("SPI device");
    let spi: SpiDeviceDriver<'static, _> = unsafe { core::mem::transmute(spi) };

    // ─── GPIO: BUSY=GPIO10  RESET=GPIO3 ───────────────────────────────────────
    let busy = PinDriver::input(p.pins.gpio10, Pull::Floating).expect("BUSY");
    let reset = PinDriver::output(p.pins.gpio3).expect("RESET");
    let busy: PinDriver<'static, _> = unsafe { core::mem::transmute(busy) };
    let reset: PinDriver<'static, _> = unsafe { core::mem::transmute(reset) };

    // ─── Init LR1121 (mismo camino que el TX; valida SPI/soldadura) ────────────
    let mut radio = match Lr1121::new(spi, busy, reset) {
        Ok(r) => {
            info!("rx: SPI OK — el LR1121 respondió.");
            r
        }
        Err(e) => {
            error!("rx: init LR1121 falló: {:?} — revisar SPI/BUSY/RESET/3V3", e);
            loop {
                FreeRtos::delay_ms(1000);
            }
        }
    };

    // ─── Configurar y arrancar RX continuo ────────────────────────────────────
    if let Err(e) = radio.configure_rx(DEFAULT_FREQ_HZ, DEFAULT_SF, DEFAULT_BW_KHZ, DEFAULT_CR) {
        error!("rx: configure_rx: {:?}", e);
    }
    if let Err(e) = radio.start_rx() {
        error!("rx: start_rx: {:?}", e);
    }

    info!(
        "rx: escuchando @ {:.3} MHz SF{} BW{} CR{} — esperando paquetes 'P2'…",
        DEFAULT_FREQ_HZ as f64 / 1e6, DEFAULT_SF, DEFAULT_BW_KHZ, DEFAULT_CR
    );

    // ─── Loop de recepción ────────────────────────────────────────────────────
    let mut stats = LinkStats::new();
    let mut last_beat = std::time::Instant::now();
    let mut rssi_max: i8 = -128;
    loop {
        // Muestrear RSSI instantánea cada vuelta; guardar el pico de la ventana.
        // Si la radio está en RX, durante los bursts del TX sube fuerte.
        let r = radio.rssi_inst();
        if r > rssi_max { rssi_max = r; }

        // Heartbeat: aunque no lleguen paquetes, mostrar que el RX está vivo y
        // escuchando, con diagnóstico de actividad (RSSI pico + flags vistos).
        if last_beat.elapsed().as_secs() >= 5 {
            last_beat = std::time::Instant::now();
            let (pre, hok, herr, crc, done) = radio.rx_seen_flags();
            info!(
                "rx: vivo @ {:.3}MHz SF{} | rcv={} lost={} per={}.{}% | rssi_pico={}dBm | visto: pre={} hdr={} hdrErr={} crc={} rxDone={}",
                DEFAULT_FREQ_HZ as f64 / 1e6, DEFAULT_SF,
                stats.received, stats.lost(),
                stats.per_permille() / 10, stats.per_permille() % 10,
                rssi_max, pre as u8, hok as u8, herr as u8, crc as u8, done as u8,
            );
            rssi_max = -128;
        }

        match radio.try_receive() {
            Ok(Some(RxPacket { payload, rssi_dbm, snr_db })) => {
                match parse_seq(&payload) {
                    Some(seq) => {
                        stats.received += 1;
                        if stats.first_seq.is_none() {
                            stats.first_seq = Some(seq);
                        }
                        stats.last_seq = seq;
                        info!(
                            "rx seq={} rssi={}dBm snr={}dB | rcv={} lost={} per={}.{}%",
                            seq, rssi_dbm, snr_db,
                            stats.received, stats.lost(),
                            stats.per_permille() / 10, stats.per_permille() % 10,
                        );
                    }
                    None => {
                        stats.unknown += 1;
                        warn!(
                            "rx: paquete desconocido ({} bytes) rssi={}dBm snr={}dB — {:02x?}",
                            payload.len(), rssi_dbm, snr_db, payload
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                stats.bad_crc += 1;
                // try_receive ya re-arma RX tras un error de CRC; sólo lo contamos.
                warn!("rx: {:?} (crc/err acumulados={})", e, stats.bad_crc);
            }
        }
        FreeRtos::delay_ms(5);
    }
}
