//! node — bring-up del LR1121 (ESP32-C3), un solo nodo.
//!
//! Pinout LR1121 → ESP32-C3:  CS=7  CLK=4  MOSI=6  MISO=5  RESET=3  BUSY=10
//!
//! PASO 1 (hecho): `Lr1121::new()` valida SPI/soldadura leyendo GetVersion.
//! PASO 2 (hecho): configura la radio a 916.8 MHz · SF7 · BW125 · CR4/5 y
//!   transmite un paquete corto cada 2 s. Requiere ANTENA 915 conectada.
//! PASO 2.5a (esto): smoke test de BLE. Levanta NimBLE, hace advertising como
//!   "LORA-NODE-01" y expone UNA característica legible (info del dispositivo).
//!   Objetivo: confirmar que el BLE arranca en esta placa y que un celular
//!   (p.ej. nRF Connect) lo ve, se conecta y puede leerla — conviviendo con el
//!   LoRa (BLE usa la radio 2.4 GHz interna del C3; el LR1121 es sub-GHz por SPI).
//!   Las características reales del protocolo (config R/W, notify de paquetes)
//!   vienen en el 2.5b.

use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    peripherals::Peripherals,
    spi::{config::Config as SpiConfig, SpiDeviceDriver, SpiDriver, SPI2},
    units::Hertz,
};
use esp_idf_svc::log::EspLogger;
use esp32_nimble::{utilities::BleUuid, BLEAdvertisementData, BLEDevice, NimbleProperties};
use log::{error, info};
use lr1121::{
    Lr1121, DEFAULT_BW_KHZ, DEFAULT_CR, DEFAULT_FREQ_HZ, DEFAULT_SF, DEFAULT_TX_DBM,
};

const TX_INTERVAL_MS: u32 = 2_000;
const PAYLOAD_LEN: u8 = 4;

// UUIDs del servicio/característica (coinciden con protocol.ts del proyecto).
const SVC: BleUuid = BleUuid::from_uuid16(0xff00);
const CHAR_INFO: BleUuid = BleUuid::from_uuid16(0xff01); // lectura

fn main() {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("node: paso 2.5a — radio + BLE smoke test");

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

    // ─── Init + configurar TX (pasos 1 y 2) ───────────────────────────────────
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

    if let Err(e) = radio.configure_tx(
        DEFAULT_FREQ_HZ,
        DEFAULT_SF,
        DEFAULT_BW_KHZ,
        DEFAULT_CR,
        DEFAULT_TX_DBM,
        PAYLOAD_LEN,
    ) {
        error!("node: configure_tx falló: {:?}", e);
        loop {
            FreeRtos::delay_ms(1000);
        }
    }

    info!(
        "node: radio lista @ {:.3} MHz SF{} BW{} CR{} {}dBm",
        DEFAULT_FREQ_HZ as f64 / 1e6,
        DEFAULT_SF,
        DEFAULT_BW_KHZ,
        DEFAULT_CR,
        DEFAULT_TX_DBM
    );

    // ─── BLE: advertising + una característica legible ─────────────────────────
    let ble = BLEDevice::take();
    let server = ble.get_server();
    let service = server.create_service(SVC);

    let char_info = service
        .lock()
        .create_characteristic(CHAR_INFO, NimbleProperties::READ);
    char_info
        .lock()
        .set_value(b"LORA-NODE-01 rust/lr1121 916.8MHz");

    ble.get_advertising()
        .lock()
        .set_data(
            BLEAdvertisementData::new()
                .name("LORA-NODE-01")
                .add_service_uuid(SVC),
        )
        .expect("ble adv data");
    ble.get_advertising().lock().start().expect("ble adv start");

    info!("node: BLE advertising como \"LORA-NODE-01\" (servicio 0xff00, char 0xff01 R)");

    // ─── Loop de TX ───────────────────────────────────────────────────────────
    let mut seq: u16 = 0;
    loop {
        let pkt = [b'P', b'2', seq as u8, (seq >> 8) as u8];
        match radio.transmit(&pkt) {
            Ok(()) => info!("tx seq={} ok", seq),
            Err(e) => error!("tx seq={} error: {:?}", seq, e),
        }
        seq = seq.wrapping_add(1);
        FreeRtos::delay_ms(TX_INTERVAL_MS);
    }
}
