//! node — nodo transmisor LoRa (ESP32-C3 + LR1121) con control por BLE GATT.
//!
//! Pinout LR1121 → ESP32-C3:  CS=7  CLK=4  MOSI=6  MISO=5  RESET=3  BUSY=10
//!
//! PASO 1 (hecho): `Lr1121::new()` valida SPI/soldadura (GetVersion).
//! PASO 2 (hecho): radio @ 916.8 MHz · SF7 · BW125 · CR4/5, TX cada 2 s.
//! PASO 2.5a (hecho): smoke test de BLE (advertising + característica legible).
//! PASO 2.5b (esto): características reales del protocolo (protocol.ts):
//!   - 0xFF01 DEVICE_INFO (R): { name, firmware, role }
//!   - 0xFF02 RADIO_CONFIG (R/W): freq/SF/BW/CR/potencia — al escribir, reconfigura
//!     la radio de verdad.
//!   - 0xFF03 RADIO_STATUS (R): { success }
//!   - 0xFF05 COMMAND (W): PAUSE_TX, RESUME_TX, SEND_ONE, SET_TX_INTERVAL,
//!     SET_RADIO_CONFIG — controlan el loop de TX en vivo.
//!   El notify de ACK (0xFF06) llega en el paso 3 (necesita el gateway que responda).
//!   Requiere ANTENA 915 conectada.

use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config as SpiConfig, SpiDeviceDriver, SpiDriver, SPI2},
    units::Hertz,
};
use esp_idf_svc::log::EspLogger;
use esp32_nimble::{utilities::BleUuid, BLEAdvertisementData, BLEDevice, NimbleProperties};
use log::{error, info, warn};
use lr1121::{
    Lr1121, DEFAULT_BW_KHZ, DEFAULT_CR, DEFAULT_FREQ_HZ, DEFAULT_SF, DEFAULT_TX_DBM,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ─── UUIDs (coinciden con protocol.ts) ────────────────────────────────────────
const SVC:         BleUuid = BleUuid::from_uuid16(0xff00);
const CHAR_INFO:   BleUuid = BleUuid::from_uuid16(0xff01); // R
const CHAR_CFG:    BleUuid = BleUuid::from_uuid16(0xff02); // R/W
const CHAR_STATUS: BleUuid = BleUuid::from_uuid16(0xff03); // R
const CHAR_CMD:    BleUuid = BleUuid::from_uuid16(0xff05); // W

const DEFAULT_INTERVAL_MS: u32 = 2_000;
const PAYLOAD_LEN: u8 = 4;

// ─── Config de radio (nombres de campo = protocol.ts) ─────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RadioConfig {
    #[serde(rename = "freqHz")]     freq_hz: u32,
    #[serde(rename = "bwKhz")]      bw_khz: u32,
                                    sf: u8,
                                    cr: String,
    #[serde(rename = "txPowerDbm")] tx_power_dbm: i8,
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self {
            freq_hz: DEFAULT_FREQ_HZ,
            sf: DEFAULT_SF,
            bw_khz: DEFAULT_BW_KHZ,
            cr: DEFAULT_CR.to_string(),
            tx_power_dbm: DEFAULT_TX_DBM,
        }
    }
}

#[derive(Deserialize)]
struct Command {
    cmd: String,
    #[serde(flatten)]
    payload: serde_json::Value,
}

struct TxState {
    running: bool,
    interval_ms: u32,
    send_one: bool,
    reconfig: Option<RadioConfig>,
    radio_config: RadioConfig,
}

impl Default for TxState {
    fn default() -> Self {
        Self {
            running: true,
            interval_ms: DEFAULT_INTERVAL_MS,
            send_one: false,
            reconfig: None,
            radio_config: RadioConfig::default(),
        }
    }
}

// Valor de característica = JSON plano (en lecturas). En escrituras, el cliente
// manda base64(JSON) y acá lo decodificamos. Mismo contrato que la app.
fn json_bytes<T: Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_string(v).unwrap_or_default().into_bytes()
}

fn decode_b64_json(data: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(data).ok()?;
    String::from_utf8(base64::decode(s.trim()).ok()?).ok()
}

fn main() {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("node: paso 2.5b — nodo TX con control por BLE");

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

    // ─── Init LR1121 ──────────────────────────────────────────────────────────
    let mut radio = match Lr1121::new(spi, busy, reset) {
        Ok(r) => {
            info!("node: SPI OK — el LR1121 respondió.");
            r
        }
        Err(e) => {
            error!("node: init LR1121 falló: {:?} — revisar SPI/BUSY/RESET/3V3", e);
            loop {
                FreeRtos::delay_ms(1000);
            }
        }
    };

    let state = Arc::new(Mutex::new(TxState::default()));

    {
        let s = state.lock().unwrap();
        if let Err(e) = radio.configure_tx(
            s.radio_config.freq_hz, s.radio_config.sf, s.radio_config.bw_khz,
            &s.radio_config.cr, s.radio_config.tx_power_dbm, PAYLOAD_LEN,
        ) {
            error!("node: configure_tx inicial: {:?}", e);
        }
    }

    info!("node: radio lista @ {:.3} MHz SF{} BW{}",
        DEFAULT_FREQ_HZ as f64 / 1e6, DEFAULT_SF, DEFAULT_BW_KHZ);

    // ─── BLE: servicio + características ───────────────────────────────────────
    let ble = BLEDevice::take();
    let server = ble.get_server();
    let service = server.create_service(SVC);

    // DEVICE_INFO (R)
    let char_info = service.lock().create_characteristic(CHAR_INFO, NimbleProperties::READ);
    char_info.lock().set_value(&json_bytes(&serde_json::json!({
        "name":     "LORA-NODE-01",
        "firmware": env!("CARGO_PKG_VERSION"),
        "role":     "node",
    })));

    // RADIO_CONFIG (R/W)
    let char_cfg = service.lock().create_characteristic(
        CHAR_CFG, NimbleProperties::READ | NimbleProperties::WRITE,
    );
    char_cfg.lock().set_value(&json_bytes(&RadioConfig::default()));

    // RADIO_STATUS (R)
    let char_status = service.lock().create_characteristic(CHAR_STATUS, NimbleProperties::READ);
    char_status.lock().set_value(&json_bytes(&serde_json::json!({ "success": true })));

    // COMMAND (W)
    let char_cmd = service.lock().create_characteristic(CHAR_CMD, NimbleProperties::WRITE);
    {
        let state_c = state.clone();
        let char_cfg_c = char_cfg.clone();
        let char_status_c = char_status.clone();

        char_cmd.lock().on_write(move |args| {
            let Some(json) = decode_b64_json(args.recv_data()) else {
                warn!("BLE: comando no es base64(JSON) válido");
                return;
            };
            let Ok(cmd) = serde_json::from_str::<Command>(&json) else {
                warn!("BLE: comando JSON inválido: {}", json);
                return;
            };

            let mut s = state_c.lock().unwrap();
            match cmd.cmd.as_str() {
                "PAUSE_TX"  => { s.running = false; info!("BLE: PAUSE_TX"); }
                "RESUME_TX" => { s.running = true;  info!("BLE: RESUME_TX"); }
                "SEND_ONE"  => { s.send_one = true;  info!("BLE: SEND_ONE"); }
                "SET_TX_INTERVAL" => {
                    if let Some(ms) = cmd.payload.get("intervalMs").and_then(|v| v.as_u64()) {
                        s.interval_ms = (ms as u32).clamp(500, 60_000);
                        info!("BLE: SET_TX_INTERVAL {}ms", s.interval_ms);
                    }
                }
                "SET_RADIO_CONFIG" => {
                    match serde_json::from_value::<RadioConfig>(cmd.payload.clone()) {
                        Ok(cfg) => {
                            info!("BLE: SET_RADIO_CONFIG {}Hz SF{}", cfg.freq_hz, cfg.sf);
                            char_cfg_c.lock().set_value(&json_bytes(&cfg));
                            char_status_c.lock().set_value(&json_bytes(&serde_json::json!({ "success": true })));
                            s.reconfig = Some(cfg);
                        }
                        Err(e) => {
                            warn!("BLE: SET_RADIO_CONFIG inválido: {}", e);
                            char_status_c.lock().set_value(&json_bytes(&serde_json::json!({ "success": false })));
                        }
                    }
                }
                other => warn!("BLE: comando desconocido: {}", other),
            }
        });
    }

    // Advertising
    ble.get_advertising().lock()
        .set_data(BLEAdvertisementData::new().name("LORA-NODE-01").add_service_uuid(SVC))
        .expect("ble adv data");
    ble.get_advertising().lock().start().expect("ble adv start");

    info!("node: BLE advertising como \"LORA-NODE-01\"");

    // ─── Loop de TX (controlado por el estado que mueve el BLE) ───────────────
    let mut seq: u16 = 0;
    loop {
        // Aplicar reconfiguración de radio pendiente.
        let reconfig = state.lock().unwrap().reconfig.take();
        if let Some(cfg) = reconfig {
            match radio.configure_tx(cfg.freq_hz, cfg.sf, cfg.bw_khz, &cfg.cr, cfg.tx_power_dbm, PAYLOAD_LEN) {
                Ok(()) => {
                    info!("node: radio reconfigurada @ {}Hz SF{}", cfg.freq_hz, cfg.sf);
                    state.lock().unwrap().radio_config = cfg;
                }
                Err(e) => error!("node: configure_tx (reconfig): {:?}", e),
            }
        }

        // Decidir si transmitir (loop automático o un SEND_ONE puntual).
        let should_tx = {
            let mut s = state.lock().unwrap();
            let one = s.send_one;
            s.send_one = false;
            s.running || one
        };

        if should_tx {
            let pkt = [b'P', b'2', seq as u8, (seq >> 8) as u8];
            match radio.transmit(&pkt) {
                Ok(()) => info!("tx seq={} ok", seq),
                Err(e) => error!("tx seq={} error: {:?}", seq, e),
            }
            seq = seq.wrapping_add(1);
            let interval = state.lock().unwrap().interval_ms;
            FreeRtos::delay_ms(interval);
        } else {
            // Pausado: yield breve para no quemar CPU.
            FreeRtos::delay_ms(50);
        }
    }
}
