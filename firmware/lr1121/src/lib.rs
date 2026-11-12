//! Driver Rust para LR1121 en modo transceiver raw.
//!
//! Pinout (ESP32-C3):  CS=7  CLK=4  MOSI=6  MISO=5  RESET=3  BUSY=10
//! DIO1 no conectado — se usa polling del registro IRQ vía SPI.
//!
//! Copia el patrón HAL de `lr1121-transceiver` del repo estacion_meteorologica.

use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{Input, Output, PinDriver},
    spi::{SpiDeviceDriver, SpiDriver},
};
use log::{debug, warn};

// ─── Constantes de canal ──────────────────────────────────────────────────────

pub const DEFAULT_FREQ_HZ: u32 = 916_800_000; // AU915 sub-band 2 ch8
pub const DEFAULT_SF: u8 = 7;
pub const DEFAULT_BW_KHZ: u32 = 125;
pub const DEFAULT_CR: &str = "4/5";
pub const DEFAULT_TX_DBM: i8 = 14;

// ─── IRQ masks ────────────────────────────────────────────────────────────────

const IRQ_TX_DONE:  u32 = 1 << 2;  // 0x000004
const IRQ_RX_DONE:  u32 = 1 << 3;  // 0x000008
const IRQ_CRC_ERR:  u32 = 1 << 7;  // 0x000080  — CRC error del paquete recibido
const IRQ_TIMEOUT:  u32 = 1 << 10; // 0x000400  — RX/TX timeout
const IRQ_CMD_ERR:  u32 = 1 << 22; // 0x400000  — comando rechazado (opcode/param inválido)
const IRQ_HW_ERR:   u32 = 1 << 23; // 0x800000  — error de HW (calibración/PLL)
// Cualquier condición de error que deba abortar una operación:
const IRQ_ERR: u32 = IRQ_CRC_ERR | IRQ_CMD_ERR | IRQ_HW_ERR;

// ─── Tipos públicos ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum Lr1121Error {
    BusyTimeout,
    CommandFailed(u8),
    TxTimeout,
    CrcError,
    NotInitialized,
    /// GetVersion devolvió un chip_type distinto del LR1121 (0x03): el SPI no
    /// lee bien (cableado/soldadura) o no es un LR1121. Lleva el valor leído.
    UnexpectedChip(u8),
}

/// chip_type que reporta GetVersion para el LR1121.
const LR1121_CHIP_TYPE: u8 = 0x03;

impl std::fmt::Display for Lr1121Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}
impl std::error::Error for Lr1121Error {}

#[derive(Debug)]
pub struct RxPacket {
    pub payload: Vec<u8>,
    pub rssi_dbm: i16,
    pub snr_db: i8,
}

// ─── HAL context (global mutable, patrón del repo existente) ──────────────────

pub(crate) struct HalCtx {
    pub spi:   SpiDeviceDriver<'static, SpiDriver<'static>>,
    pub busy:  PinDriver<'static, Input>,
    pub reset: PinDriver<'static, Output>,
}

pub(crate) static mut HAL_CTX: *mut HalCtx = core::ptr::null_mut();

const BUSY_POLL_MS: u32 = 1;
const BUSY_MAX_ITERS: u32 = 1_000;

unsafe fn wait_busy_low() -> bool {
    let ctx = &mut *HAL_CTX;
    for _ in 0..BUSY_MAX_ITERS {
        if ctx.busy.is_low() { return true; }
        FreeRtos::delay_ms(BUSY_POLL_MS);
    }
    warn!("lr1121: BUSY timeout");
    false
}

// ─── HAL callbacks llamados por SWDR001 ───────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_write(
    _ctx: *const core::ffi::c_void,
    command: *const u8, command_length: u16,
    data: *const u8, data_length: u16,
) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let cmd = core::slice::from_raw_parts(command, command_length as usize);
    let payload = if data_length > 0 { core::slice::from_raw_parts(data, data_length as usize) } else { &[] };
    let mut buf = Vec::with_capacity(cmd.len() + payload.len());
    buf.extend_from_slice(cmd);
    buf.extend_from_slice(payload);
    if ctx.spi.write(&buf).is_err() { return 1; }
    if !wait_busy_low() { return 2; }
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_read(
    _ctx: *const core::ffi::c_void,
    command: *const u8, command_length: u16,
    data: *mut u8, data_length: u16,
) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let cmd = core::slice::from_raw_parts(command, command_length as usize);
    if ctx.spi.write(cmd).is_err() { return 1; }
    if !wait_busy_low() { return 2; }
    // The LR11xx SPI read response always starts with one STAT1 byte before the actual data.
    // We must read data_length+1 bytes in a single CS assertion and discard STAT1 (index 0).
    // Reading only data_length bytes would cause every response to be shifted by one byte,
    // making IRQ status, version, and all other reads return wrong values.
    let total = data_length as usize + 1;
    let mut buf = vec![0u8; total];
    if ctx.spi.read(&mut buf).is_err() { return 1; }
    if data_length > 0 {
        let rx = core::slice::from_raw_parts_mut(data, data_length as usize);
        rx.copy_from_slice(&buf[1..]);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_write_read(
    _ctx: *const core::ffi::c_void,
    command: *const u8, data: *mut u8, length: u16,
) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let mut buf: Vec<u8> = core::slice::from_raw_parts(command, length as usize).to_vec();
    if ctx.spi.transfer_in_place(&mut buf).is_err() { return 1; }
    core::slice::from_raw_parts_mut(data, length as usize).copy_from_slice(&buf);
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_reset(_ctx: *const core::ffi::c_void) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let _ = ctx.reset.set_low();
    FreeRtos::delay_ms(10);
    let _ = ctx.reset.set_high();
    FreeRtos::delay_ms(10); // espera boot transceiver
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_wakeup(_ctx: *const core::ffi::c_void) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let _ = ctx.spi.write(&[0x00]);
    FreeRtos::delay_ms(2);
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_direct_read(
    _ctx: *const core::ffi::c_void,
    data: *mut u8, data_length: u16,
) -> u8 {
    if HAL_CTX.is_null() { return 1; }
    let ctx = &mut *HAL_CTX;
    let rx = core::slice::from_raw_parts_mut(data, data_length as usize);
    if ctx.spi.read(rx).is_err() { return 1; }
    0
}

#[no_mangle]
pub unsafe extern "C" fn lr11xx_hal_abort_blocking_cmd(_ctx: *const core::ffi::c_void) -> u8 {
    0 // no-op: no usamos comandos bloqueantes
}

// ─── Tipos C para SWDR001 ─────────────────────────────────────────────────────

#[repr(C)]
#[derive(Default, Debug)]
struct Lr11xxSystemVersion {
    chip_firmware_type: u8,
    fw_version: u16,
    chip_type: u8,
}

// IMPORTANTE: los campos enum de C son `int` (4 bytes), no `uint8_t`. SWDR001 se
// compila sin -fshort-enums, así que estas structs DEBEN usar u32 para los enums
// o el chip lee los parámetros de offsets equivocados y rechaza el comando
// (CMD_ERROR, IRQ bit 22). `#[repr(C)]` reproduce el mismo padding que C.
#[repr(C)]
struct Lr11xxRadioLoraModParams {
    sf: u32,  // SpreadingFactor enum (SF7=0x07 … SF12=0x0C)
    bw: u32,  // Bandwidth enum (125k=0x04, 250k=0x05, 500k=0x06)
    cr: u32,  // CodingRate enum (4/5=0x01, 4/6=0x02, 4/7=0x03, 4/8=0x04)
    ldro: u8, // Low Data Rate Optimization (0=off, 1=on) — uint8_t en C
}

#[repr(C)]
struct Lr11xxRadioPaCfg {
    pa_sel: u32,        // enum: 0=LP, 1=HP, 2=HF
    pa_reg_supply: u32, // enum: 0=VREG, 1=VBAT
    pa_duty_cycle: u8,  // uint8_t: 0–7
    pa_hp_sel: u8,      // uint8_t: 0–7 (solo HP PA)
}

#[repr(C)]
struct Lr11xxRadioLoraPktParams {
    preamble_len: u16, // uint16_t: símbolos
    header_type: u32,  // enum: 0=EXPLICIT
    pld_len: u8,       // uint8_t: payload bytes
    crc: u32,          // enum: 0=OFF, 1=ON
    iq: u32,           // enum: 0=STANDARD
}

#[repr(C)]
#[derive(Default)]
struct Lr11xxRadioLoraPacketStatus {
    rssi_pkt_in_dbm: i8,
    snr_pkt_in_db: i8,
    signal_rssi_pkt_in_dbm: i8,
}

#[repr(C)]
#[derive(Default)]
struct Lr11xxRadioRxBufferStatus {
    payload_length: u8,
    buffer_start_pointer: u8,
}

// ─── Extern "C" — funciones SWDR001 ──────────────────────────────────────────

extern "C" {
    fn lr11xx_system_reset(ctx: *const core::ffi::c_void) -> u8;
    fn lr11xx_system_get_version(ctx: *const core::ffi::c_void, v: *mut Lr11xxSystemVersion) -> u8;

    fn lr11xx_radio_set_pkt_type(ctx: *const core::ffi::c_void, pkt_type: u8) -> u8;
    fn lr11xx_radio_set_rf_freq(ctx: *const core::ffi::c_void, freq_hz: u32) -> u8;
    fn lr11xx_radio_set_lora_mod_params(ctx: *const core::ffi::c_void, p: *const Lr11xxRadioLoraModParams) -> u8;

    // TX
    fn lr11xx_radio_set_pa_cfg(ctx: *const core::ffi::c_void, p: *const Lr11xxRadioPaCfg) -> u8;
    fn lr11xx_radio_set_tx_params(ctx: *const core::ffi::c_void, pwr_dbm: i8, ramp_time: u8) -> u8;
    fn lr11xx_radio_set_lora_pkt_params(ctx: *const core::ffi::c_void, p: *const Lr11xxRadioLoraPktParams) -> u8;
    fn lr11xx_regmem_write_buffer8(ctx: *const core::ffi::c_void, buf: *const u8, len: u8) -> u8;
    fn lr11xx_radio_set_tx(ctx: *const core::ffi::c_void, timeout_ms: u32) -> u8;

    // RX
    fn lr11xx_radio_set_rx(ctx: *const core::ffi::c_void, timeout_ms: u32) -> u8;
    fn lr11xx_radio_get_rx_buffer_status(ctx: *const core::ffi::c_void, s: *mut Lr11xxRadioRxBufferStatus) -> u8;
    fn lr11xx_radio_get_lora_pkt_status(ctx: *const core::ffi::c_void, s: *mut Lr11xxRadioLoraPacketStatus) -> u8;
    fn lr11xx_regmem_read_buffer8(ctx: *const core::ffi::c_void, buf: *mut u8, len: u8) -> u8;

    // IRQ / errores
    fn lr11xx_system_get_and_clear_irq_status(ctx: *const core::ffi::c_void, irq: *mut u32) -> u8;
    fn lr11xx_system_get_errors(ctx: *const core::ffi::c_void, errors: *mut u16) -> u8;
    fn lr11xx_system_clear_errors(ctx: *const core::ffi::c_void) -> u8;

    // Reloj de RF (TCXO) + calibración
    fn lr11xx_system_set_tcxo_mode(ctx: *const core::ffi::c_void, tune: u8, timeout: u32) -> u8;
    fn lr11xx_system_calibrate(ctx: *const core::ffi::c_void, calib_param: u8) -> u8;
}

// TCXO: voltaje de supply (enum LR11XX_SYSTEM_TCXO_CTRL_*). 0x02 = 1.8 V.
const TCXO_CTRL_1_8V: u8 = 0x02;
// Timeout de arranque del TCXO en pasos de RTC (30.52 µs). 164 ≈ 5 ms.
const TCXO_TIMEOUT_RTC: u32 = 164;
// Calibrar todos los bloques (LF_RC|HF_RC|PLL|ADC|IMG|PLL_TX).
const CALIB_ALL: u8 = 0x3F;

/// Lee el registro de errores del LR11xx (bitfield: LF/HF RC calib, ADC, PLL
/// calib, IMG calib, HF/LF XOSC start, PLL lock). 0 = sin errores.
fn read_errors() -> u16 {
    let mut e: u16 = 0;
    unsafe { lr11xx_system_get_errors(HAL_CTX as *const _, &mut e) };
    e
}

// ─── Helper: convierte parámetros de la app al formato del driver ─────────────

pub fn sf_to_u8(sf: u8) -> u8 { sf } // SF7=0x07 … SF12=0x0C (LR11xx SDK direct mapping)

pub fn bw_khz_to_u8(bw: u32) -> u8 {
    match bw { 250 => 0x05, 500 => 0x06, _ => 0x04 } // default 125 kHz
}

pub fn cr_str_to_u8(cr: &str) -> u8 {
    match cr { "4/6" => 0x02, "4/7" => 0x03, "4/8" => 0x04, _ => 0x01 } // default 4/5
}

/// LDRO se activa cuando el SF y BW implican symbol duration > ~16 ms.
pub fn ldro(sf: u8, bw_khz: u32) -> u8 {
    let sym_ms = (1u32 << sf) * 1000 / bw_khz;
    if sym_ms > 16 { 1 } else { 0 }
}

// ─── Safe API ─────────────────────────────────────────────────────────────────

pub struct Lr1121 {
    _ctx: Box<HalCtx>,
}

impl Lr1121 {
    /// Inicializa el chip. Llama una vez al inicio.
    pub fn new(
        spi:   SpiDeviceDriver<'static, SpiDriver<'static>>,
        busy:  PinDriver<'static, Input>,
        reset: PinDriver<'static, Output>,
    ) -> Result<Self, Lr1121Error> {
        let ctx = Box::new(HalCtx { spi, busy, reset });
        unsafe { HAL_CTX = &*ctx as *const HalCtx as *mut HalCtx; }

        unsafe {
            if lr11xx_system_reset(HAL_CTX as *const _) != 0 {
                return Err(Lr1121Error::CommandFailed(1));
            }
        }
        FreeRtos::delay_ms(20);

        // GetVersion leído y parseado en Rust desde los bytes crudos. NO usamos la
        // struct FFI lr11xx_system_version_t: su campo `type` es un enum C, cuyo
        // tamaño/offset depende del ABI del target (-fshort-enums). En C3 (riscv,
        // enum=4B) el offset coincidía por casualidad y andaba; en ESP32 (xtensa,
        // enum=1B) se corría y chip_type se leía como 0x00 con el chip sano.
        //
        // Además esperamos BUSY bajo (chip listo tras el reset) ANTES de mandar el
        // comando: si se manda mientras el chip todavía bootea, la respuesta sale
        // corrida. Leemos la trama cruda [STAT1, hw, type, fw_hi, fw_lo].
        let ver = unsafe {
            let c = &mut *HAL_CTX;
            for _ in 0..1000 { if c.busy.is_low() { break; } FreeRtos::delay_ms(1); }
            let _ = c.spi.write(&[0x01u8, 0x01u8]);
            for _ in 0..1000 { if c.busy.is_low() { break; } FreeRtos::delay_ms(1); }
            let mut buf = [0u8; 6];
            let _ = c.spi.read(&mut buf);
            buf
        };
        let chip_type = ver[2];
        let fw = ((ver[3] as u16) << 8) | ver[4] as u16;
        log::info!("lr1121 chip_type={:#04x} hw={:#04x} stat1={:#04x} fw={:#06x}",
            chip_type, ver[1], ver[0], fw);

        // Validar la identidad: si el chip_type no es 0x03, el SPI no está
        // leyendo bien (cableado/soldadura) y seguir solo da errores río abajo
        // (calibración/TX). Cortar acá con un error claro en vez de un "OK" falso.
        if chip_type != LR1121_CHIP_TYPE {
            warn!("lr1121: chip_type inesperado {:#04x} (esperado {:#04x}) — revisar SPI/soldadura",
                chip_type, LR1121_CHIP_TYPE);
            return Err(Lr1121Error::UnexpectedChip(chip_type));
        }

        // Reloj de RF: el módulo trae un TCXO que el chip debe alimentar por VTCXO.
        // Sin esto el oscilador de 32 MHz no arranca al transmitir (error
        // HF_XOSC_START). Hay que configurarlo ANTES de calibrar.
        unsafe {
            check(lr11xx_system_set_tcxo_mode(HAL_CTX as *const _, TCXO_CTRL_1_8V, TCXO_TIMEOUT_RTC))?;
            // Recalibrar todos los bloques ahora que el reloj de RF es el correcto.
            check(lr11xx_system_calibrate(HAL_CTX as *const _, CALIB_ALL))?;
            // Limpiar errores latcheados del arranque para que get_errors refleje
            // solo el estado post-configuración.
            lr11xx_system_clear_errors(HAL_CTX as *const _);
            // OJO: clear_errors limpia el registro de ERRORES, no el de IRQ status.
            // La calibración deja un CMD_ERROR latcheado (bit 22) en el IRQ status;
            // si no se limpia acá, el primer transmit() lo lee como fallo espúreo.
            let mut stale_irq: u32 = 0;
            lr11xx_system_get_and_clear_irq_status(HAL_CTX as *const _, &mut stale_irq);
        }
        FreeRtos::delay_ms(5);
        let errs = read_errors();
        if errs != 0 {
            warn!("lr1121: errores tras TCXO+calibrate = {:#06x} (bit5=HF_XOSC_START)", errs);
        } else {
            log::info!("lr1121: TCXO 1.8V + calibración OK, sin errores");
        }

        Ok(Lr1121 { _ctx: ctx })
    }

    /// Configura la radio para TX en los parámetros indicados.
    pub fn configure_tx(
        &mut self,
        freq_hz: u32,
        sf: u8,
        bw_khz: u32,
        cr: &str,
        power_dbm: i8,
        payload_len: u8,
    ) -> Result<(), Lr1121Error> {
        unsafe {
            // Packet type = LoRa (0x02)
            check(lr11xx_radio_set_pkt_type(HAL_CTX as *const _, 0x02))?;
            check(lr11xx_radio_set_rf_freq(HAL_CTX as *const _, freq_hz))?;

            let mp = Lr11xxRadioLoraModParams {
                sf: sf_to_u8(sf) as u32,
                bw: bw_khz_to_u8(bw_khz) as u32,
                cr: cr_str_to_u8(cr) as u32,
                ldro: ldro(sf, bw_khz),
            };
            check(lr11xx_radio_set_lora_mod_params(HAL_CTX as *const _, &mp))?;

            // HP PA (pa_sel=1): la antena de ESTE módulo (CORE1121) cuelga de
            // RFO_HP_LF (ver CONEXIONADO-LR1121.md). Con LP PA (pa_sel=0) la RF sale
            // por RFO_LP, que NO está cableado a la antena → TX_DONE pero NO radia.
            // El HP PA EXIGE supply VBAT (pa_reg_supply=1); con VREG dispara
            // sobrecorriente y aborta el TX.
            let pa = Lr11xxRadioPaCfg { pa_sel: 1, pa_reg_supply: 1, pa_duty_cycle: 4, pa_hp_sel: 7 };
            check(lr11xx_radio_set_pa_cfg(HAL_CTX as *const _, &pa))?;

            // TX power + ramp 200 µs (0x02). Enlace corto: no subir de +14 dBm
            // (front-end RX del LR1121 aguanta máx 0 dBm; a 4 m llega ~-30 dBm).
            check(lr11xx_radio_set_tx_params(HAL_CTX as *const _, power_dbm, 0x02))?;

            // Packet params
            let pp = Lr11xxRadioLoraPktParams {
                preamble_len: 8,
                header_type: 0,   // EXPLICIT
                pld_len: payload_len,
                crc: 1,           // CRC ON
                iq: 0,            // STANDARD IQ
            };
            check(lr11xx_radio_set_lora_pkt_params(HAL_CTX as *const _, &pp))?;
        }
        Ok(())
    }

    /// Configura la radio para RX continuo.
    pub fn configure_rx(
        &mut self,
        freq_hz: u32,
        sf: u8,
        bw_khz: u32,
        cr: &str,
    ) -> Result<(), Lr1121Error> {
        unsafe {
            check(lr11xx_radio_set_pkt_type(HAL_CTX as *const _, 0x02))?;
            check(lr11xx_radio_set_rf_freq(HAL_CTX as *const _, freq_hz))?;

            let mp = Lr11xxRadioLoraModParams {
                sf: sf_to_u8(sf) as u32,
                bw: bw_khz_to_u8(bw_khz) as u32,
                cr: cr_str_to_u8(cr) as u32,
                ldro: ldro(sf, bw_khz),
            };
            check(lr11xx_radio_set_lora_mod_params(HAL_CTX as *const _, &mp))?;

            // Packet params must match what the TX side sends:
            // EXPLICIT header so length/CR/CRC come from the header itself,
            // CRC ON and STANDARD IQ to match the node's configure_tx settings.
            let pp = Lr11xxRadioLoraPktParams {
                preamble_len: 8,
                header_type: 0,   // EXPLICIT
                pld_len: 255,     // ignored in EXPLICIT mode; set to max as a safe default
                crc: 1,           // ON
                iq: 0,            // STANDARD
            };
            check(lr11xx_radio_set_lora_pkt_params(HAL_CTX as *const _, &pp))?;
        }
        Ok(())
    }

    /// Inicia modo RX continuo. Llamar después de configure_rx().
    pub fn start_rx(&mut self) -> Result<(), Lr1121Error> {
        unsafe { check(lr11xx_radio_set_rx(HAL_CTX as *const _, 0xFFFFFF)) }
    }

    /// Transmite `payload`. Bloquea hasta TX_DONE o timeout real de 5 s.
    ///
    /// El primer set_tx tras configurar la radio puede devolver un CMD_ERROR
    /// espúreo (quirk del LR1121); el reintento siempre funciona —el poll del
    /// primer intento deja el IRQ limpio— así que reintentamos una vez.
    pub fn transmit(&mut self, payload: &[u8]) -> Result<(), Lr1121Error> {
        for attempt in 0..2u8 {
            unsafe {
                check(lr11xx_regmem_write_buffer8(HAL_CTX as *const _, payload.as_ptr(), payload.len() as u8))?;
                check(lr11xx_radio_set_tx(HAL_CTX as *const _, 0))?; // sin timeout de radio
            }

            // Deadline por reloj real (no por conteo de vueltas: cada poll puede
            // tardar más de lo nominal y falsear el timeout).
            let start = std::time::Instant::now();
            let deadline = std::time::Duration::from_millis(5_000);
            let mut irq: u32 = 0;
            let result = loop {
                FreeRtos::delay_ms(5);
                unsafe { lr11xx_system_get_and_clear_irq_status(HAL_CTX as *const _, &mut irq) };
                if irq & IRQ_TX_DONE != 0 { break Ok(()); }
                if irq & IRQ_ERR != 0 { break Err(Lr1121Error::CommandFailed((irq >> 16) as u8)); }
                if start.elapsed() >= deadline { break Err(Lr1121Error::TxTimeout); }
            };

            match result {
                Ok(()) => return Ok(()),
                // CMD_ERROR en el primer intento → reintentar una vez.
                Err(Lr1121Error::CommandFailed(_)) if attempt == 0 => {
                    FreeRtos::delay_ms(10);
                    continue;
                }
                Err(e) => {
                    warn!("transmit: fallo irq={:#010x} errores={:#06x}", irq, read_errors());
                    return Err(e);
                }
            }
        }
        unreachable!()
    }

    /// Non-blocking: retorna un paquete si hay uno disponible, None si no.
    /// Después de retornar un paquete, re-arranca RX continuo.
    pub fn try_receive(&mut self) -> Result<Option<RxPacket>, Lr1121Error> {
        let mut irq: u32 = 0;
        unsafe { lr11xx_system_get_and_clear_irq_status(HAL_CTX as *const _, &mut irq) };

        if irq & IRQ_ERR != 0 {
            warn!("lr1121: error en paquete recibido (CRC/header) irq={:#010x}", irq);
            unsafe { lr11xx_radio_set_rx(HAL_CTX as *const _, 0xFFFFFF) };
            return Err(Lr1121Error::CrcError);
        }

        if irq & IRQ_RX_DONE == 0 { return Ok(None); }

        // Leer estado del paquete
        let mut pkt_st = Lr11xxRadioLoraPacketStatus::default();
        unsafe { lr11xx_radio_get_lora_pkt_status(HAL_CTX as *const _, &mut pkt_st) };

        let mut buf_st = Lr11xxRadioRxBufferStatus::default();
        unsafe {
            check(lr11xx_radio_get_rx_buffer_status(HAL_CTX as *const _, &mut buf_st))?;
        }

        let len = buf_st.payload_length as usize;
        let mut payload = vec![0u8; len];
        unsafe {
            check(lr11xx_regmem_read_buffer8(HAL_CTX as *const _, payload.as_mut_ptr(), len as u8))?;
            lr11xx_radio_set_rx(HAL_CTX as *const _, 0xFFFFFF); // re-arm RX
        }

        Ok(Some(RxPacket {
            payload,
            rssi_dbm: pkt_st.rssi_pkt_in_dbm as i16,
            snr_db: pkt_st.snr_pkt_in_db,
        }))
    }

    /// Transmite ACK downlink y vuelve a RX continuo.
    /// Usado por el gateway justo después de recibir un paquete del nodo.
    pub fn transmit_ack_and_return_rx(
        &mut self,
        ack: &[u8],
        freq_hz: u32, sf: u8, bw_khz: u32, cr: &str, power_dbm: i8,
    ) -> Result<(), Lr1121Error> {
        self.configure_tx(freq_hz, sf, bw_khz, cr, power_dbm, ack.len() as u8)?;
        self.transmit(ack)?;
        self.configure_rx(freq_hz, sf, bw_khz, cr)?;
        self.start_rx()
    }

    /// Configura RX y espera hasta `timeout_ms` por un paquete.
    /// Errores CRC se tratan como ausencia de paquete (Ok(None)).
    /// Deja la radio en standby al retornar.
    pub fn receive_with_timeout_ms(
        &mut self,
        freq_hz: u32, sf: u8, bw_khz: u32, cr: &str,
        timeout_ms: u32,
    ) -> Result<Option<RxPacket>, Lr1121Error> {
        self.configure_rx(freq_hz, sf, bw_khz, cr)?;
        unsafe { check(lr11xx_radio_set_rx(HAL_CTX as *const _, 0))? }

        let mut elapsed = 0u32;
        loop {
            FreeRtos::delay_ms(10);
            elapsed += 10;

            let mut irq: u32 = 0;
            unsafe { lr11xx_system_get_and_clear_irq_status(HAL_CTX as *const _, &mut irq) };

            if irq & (IRQ_ERR | IRQ_TIMEOUT) != 0 { return Ok(None); }

            if irq & IRQ_RX_DONE != 0 {
                let mut pkt_st = Lr11xxRadioLoraPacketStatus::default();
                unsafe { lr11xx_radio_get_lora_pkt_status(HAL_CTX as *const _, &mut pkt_st) };
                let mut buf_st = Lr11xxRadioRxBufferStatus::default();
                unsafe { check(lr11xx_radio_get_rx_buffer_status(HAL_CTX as *const _, &mut buf_st))? };
                let len = buf_st.payload_length as usize;
                let mut payload = vec![0u8; len];
                unsafe {
                    check(lr11xx_regmem_read_buffer8(HAL_CTX as *const _, payload.as_mut_ptr(), len as u8))?;
                }
                return Ok(Some(RxPacket {
                    payload,
                    rssi_dbm: pkt_st.rssi_pkt_in_dbm as i16,
                    snr_db: pkt_st.snr_pkt_in_db,
                }));
            }

            if elapsed >= timeout_ms { return Ok(None); }
        }
    }
}

impl Drop for Lr1121 {
    fn drop(&mut self) {
        unsafe { HAL_CTX = core::ptr::null_mut(); }
    }
}

#[inline]
fn check(rc: u8) -> Result<(), Lr1121Error> {
    if rc == 0 { Ok(()) } else { Err(Lr1121Error::CommandFailed(rc)) }
}

// ─── CRC-8/MAXIM (Dallas 1-Wire) ─────────────────────────────────────────────

/// Computa CRC-8/MAXIM (poly=0x31, init=0x00, refIn=true, refOut=true).
pub fn crc8_maxim(data: &[u8]) -> u8 {
    let mut crc: u8 = 0x00;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x01 != 0 { (crc >> 1) ^ 0x8C } else { crc >> 1 };
        }
    }
    crc
}

// ─── Packet encoding / decoding (formato CLAUDE.md) ──────────────────────────

/// Serializa datos de sensores en los 9 bytes del protocolo.
pub fn encode_packet(seq: u16, temp_c100: i16, hum_c100: u16, bat_mv: u16) -> [u8; 9] {
    let mut pkt = [0u8; 9];
    pkt[0..2].copy_from_slice(&seq.to_le_bytes());
    pkt[2..4].copy_from_slice(&temp_c100.to_le_bytes());
    pkt[4..6].copy_from_slice(&hum_c100.to_le_bytes());
    pkt[6..8].copy_from_slice(&bat_mv.to_le_bytes());
    pkt[8] = crc8_maxim(&pkt[0..8]);
    pkt
}

/// Decodifica y verifica CRC. Retorna (seq, temp_c100, hum_c100, bat_mv).
pub fn decode_packet(raw: &[u8]) -> Option<(u16, i16, u16, u16)> {
    if raw.len() < 9 { return None; }
    if crc8_maxim(&raw[0..8]) != raw[8] { return None; }
    Some((
        u16::from_le_bytes([raw[0], raw[1]]),
        i16::from_le_bytes([raw[2], raw[3]]),
        u16::from_le_bytes([raw[4], raw[5]]),
        u16::from_le_bytes([raw[6], raw[7]]),
    ))
}

// ─── ACK downlink encoding / decoding ────────────────────────────────────────
// Format (7 bytes): seqAck(u16le) | rssiGw(i16le) | snrGw(i8) | delivered(u8) | crc8

/// Codifica un paquete ACK downlink (gateway → nodo, 7 bytes).
pub fn encode_ack(seq: u16, rssi_dbm: i16, snr_db: i8, delivered: bool) -> [u8; 7] {
    let mut buf = [0u8; 7];
    buf[0..2].copy_from_slice(&seq.to_le_bytes());
    buf[2..4].copy_from_slice(&rssi_dbm.to_le_bytes());
    buf[4] = snr_db as u8;
    buf[5] = delivered as u8;
    buf[6] = crc8_maxim(&buf[0..6]);
    buf
}

/// Decodifica un paquete ACK downlink. Retorna (seqAck, rssiGw, snrGw, delivered).
pub fn decode_ack(raw: &[u8]) -> Option<(u16, i16, i8, bool)> {
    if raw.len() < 7 { return None; }
    if crc8_maxim(&raw[0..6]) != raw[6] { return None; }
    Some((
        u16::from_le_bytes([raw[0], raw[1]]),
        i16::from_le_bytes([raw[2], raw[3]]),
        raw[4] as i8,
        raw[5] != 0,
    ))
}
