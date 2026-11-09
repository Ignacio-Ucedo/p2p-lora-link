# firmware — nodos ESP32-C3 + LR1121

Firmware **Rust (esp-idf)** de los nodos del enlace LoRa P2P.

El driver del LR1121 y la configuración de build se reutilizan del proyecto
hermano (estación meteorológica): es un driver ya validado en hardware, que
envuelve por FFI el driver oficial de Semtech **SWDR001** (vendorizado en
`lr1121/vendor/`).

## Estructura

```
firmware/
├── Cargo.toml            # workspace
├── .cargo/config.toml    # target riscv32imc-esp-espidf, ldproxy, espflash
├── rust-toolchain.toml   # canal `esp`
├── lr1121/               # driver LR1121 (Rust + FFI a SWDR001)
│   ├── src/lib.rs
│   └── vendor/           # SWDR001 (Lora-net), driver C de Semtech
└── node/                 # binario del nodo: SPI + radio @915 + TX + BLE GATT
    ├── src/main.rs
    └── sdkconfig.defaults
```

## Pinout LR1121 → ESP32-C3

| LR1121 | ESP32-C3 |
|--------|----------|
| CS     | GPIO7    |
| CLK    | GPIO4    |
| MOSI   | GPIO6    |
| MISO   | GPIO5    |
| RESET  | GPIO3    |
| BUSY   | GPIO10   |

Detalle completo y recaudos en `CONEXIONADO-LR1121.md` (no versionado).

## Toolchain (una sola vez)

```bash
espup install -r          # el -r trae el GCC de RISC-V; sin él la build del C3 falla
. ~/export-esp.sh         # sourcear en cada terminal (o agregarlo al ~/.bashrc)
```

## El nodo (`node`)

Un solo binario que hace todo el nodo:

1. **Init LR1121** (`Lr1121::new`): resetea, lee GetVersion y **valida
   `chip_type == 0x03`**. Si el SPI no lee bien (cableado/soldadura), falla acá
   con `UnexpectedChip(..)` en vez de seguir y dar errores río abajo.
2. **TCXO + calibración** (1.8 V) — imprescindible o el oscilador de RF no
   arranca (`HF_XOSC_START`).
3. **Radio @ 916.8 MHz · SF7 · BW125 · CR4/5 · 14 dBm** y **TX** de un paquete
   corto cada 2 s.
4. **BLE GATT** (NimBLE) — advertising `LORA-NODE-01` + características de control.

### Características BLE (UUIDs = `app/lib/protocol.ts`)

| UUID   | Prop  | Qué |
|--------|-------|-----|
| 0xFF01 | R     | device info `{ name, firmware, role }` |
| 0xFF02 | R/W   | radio config `{ freqHz, bwKhz, sf, cr, txPowerDbm }` — al escribir, reconfigura la radio |
| 0xFF03 | R     | status `{ success }` |
| 0xFF05 | W     | comandos: `PAUSE_TX`, `RESUME_TX`, `SEND_ONE`, `SET_TX_INTERVAL`, `SET_RADIO_CONFIG` |

El valor se lee/escribe como JSON (en escrituras, base64 del JSON). El notify de
ACK (0xFF06) llega con el gateway/RX (paso 3).

### Flashear

> ⚠️ **Antena de 915 conectada** antes de flashear: el nodo transmite y TX sin
> antena daña el PA del LR1121.

```bash
cd firmware
. ~/export-esp.sh
cargo build --release -p node
espflash flash --monitor target/riscv32imc-esp-espidf/release/node
```

Qué deberías ver:

```
lr1121 chip_type=0x03 fw_type=34 fw=0x0000     <- SPI/soldadura OK
lr1121: TCXO 1.8V + calibración OK, sin errores
node: SPI OK — el LR1121 respondió.
node: BLE advertising como "LORA-NODE-01"
tx seq=0 ok
tx seq=1 ok
...
```

- `chip_type` distinto de `0x03` o `UnexpectedChip` → revisar SPI (CS7/CLK4/
  MOSI6/MISO5), que BUSY (GPIO10) no quede trabado y RESET (GPIO3) cableado.
  **Mirar puentes entre pines adyacentes del C3.**
- Para probar el control BLE: conectar con nRF Connect (o la app), leer 0xFF02,
  escribir un comando en 0xFF05 y re-leer 0xFF02.

## Pendiente

- Terminar el round-trip de config por BLE (escribir `SET_RADIO_CONFIG` → re-leer).
- Paso 3: segundo nodo + RX para métricas reales (RSSI/SNR/PER) y el notify de ACK.
