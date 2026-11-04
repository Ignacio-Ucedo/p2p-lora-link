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
└── node/                 # binario de bring-up (un solo nodo, sin BLE)
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

## Bring-up de un nodo (`node`)

Un hito por flasheo, para aislar de dónde viene un fallo.

**Paso 1 (el código actual): ¿el chip responde por SPI?**
Resetea el LR1121 y lee su identidad con GetVersion. Valida toda la soldadura
(SPI, RESET, BUSY) con una sola placa. **No hay RF: no transmite y no hace falta
la antena.**

```bash
cd firmware
. ~/export-esp.sh
cargo build --release -p node
espflash flash --monitor target/riscv32imc-esp-espidf/release/node
```

Qué deberías ver:

```
node: paso 1 — probando SPI al LR1121 (sin RF)
lr1121 chip_type=0x03 fw_type=... fw=0x....   <- el chip respondió
node: SPI OK — el LR1121 respondió
```

Si en cambio loguea `el LR1121 NO respondió` → revisar SPI (CS7/CLK4/MOSI6/
MISO5), que BUSY (GPIO10) no quede siempre alto y que RESET (GPIO3) esté
cableado, no al aire.

**Paso 2 (después, cuando el paso 1 pase):** agregar configuración de radio
(916.8 MHz · SF7 · BW125 · CR4/5) y TX. Recién ahí hace falta la **antena de
915 conectada** (TX sin antena daña el PA del LR1121).
