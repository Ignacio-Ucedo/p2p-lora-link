# Bring-up del LR1121 — notas y post-mortem

Notas de la puesta en marcha de los nodos ESP32 + LR1121, incluido el bug que
nos tuvo trabados y cómo se resolvió.

## Estado de los nodos

| Nodo | MCU | Puerto | Estado |
|------|-----|--------|--------|
| 1 | ESP32-C3 (SuperMini) | `/dev/ttyACM0` | Sano. Firmware real (rama `rust-lr1121`). |
| 2 | ESP32 clásico (devkit) | `/dev/ttyUSB0` | Sano, improvisado en ESP32. Rama `esp32-devkit-node`. |
| 3 | — | — | Módulo LR1121 de repuesto, sin armar. Casi seguro estaba sano también. |

> El nodo 2 nació como ESP32 clásico porque no había un C3 de repuesto para
> descartar la hipótesis (equivocada) de "C3 dañado". Ver `esp32-devkit-node`.

## El bug: `chip_type=0x00` (ABI de enum C en el FFI)

**Síntoma:** el init del LR1121 leía `chip_type=0x00 fw_type=19` y fallaba con
`UnexpectedChip(0)` en el nodo 2 / ESP32, mientras el nodo 1 (C3) leía `0x03` y
andaba. Mismo binario fuente.

**Lo que NO era (todas pistas falsas):** el chip, la soldadura (varios reflows),
los módulos (tres distintos fallaron igual), el C3, el cableado, la velocidad de
SPI, la alimentación, el reset, el BUSY. El hardware siempre estuvo sano.

**Causa raíz:** la struct FFI `Lr11xxSystemVersion` (en `firmware/lr1121/src/lib.rs`)
no espejaba bien la struct C de SWDR001 `lr11xx_system_version_t`, cuyo campo
`type` es un **enum C**. El tamaño de un enum C depende del ABI del target
(`-fshort-enums`):

- **C3 / riscv:** enum = 4 bytes → `type` cae en offset 4, y la struct Rust (mal
  escrita) lo leía justo ahí **por casualidad** → `0x03` correcto.
- **ESP32 / xtensa:** enum = 1 byte → el offset se corre → Rust leía padding →
  `0x00`, con el chip perfectamente sano.

**Cómo se diagnosticó:** se volcaron los bytes **crudos** de GetVersion (en vez
de la versión ya parseada). Dieron `[07, 22, 03, 01, 01]` **idénticos** en los dos
nodos — el `0x03` estaba ahí en ambos. Eso probó que el chip estaba bien y que el
parseo era el roto.

**Fix** (en `Lr1121::new`, `firmware/lr1121/src/lib.rs`):
1. Parsear los bytes crudos de GetVersion en Rust (`chip_type = buf[2]`), sin
   pasar por la struct del enum.
2. Esperar **BUSY bajo antes de mandar GetVersion** (si se manda mientras el chip
   bootea, la respuesta sale corrida).

**Lección:** en el FFI a SWDR001, los `enum` C son int con tamaño dependiente del
ABI. Nunca espejarlos como campos Rust angostos ni asumir offsets — parsear crudo
o usar `u32`. El código ya lo advertía para otras structs (los ModParams usan
`u32`); a la de versión se le escapó.

## Pendientes

- Llevar el fix de `lib.rs` a `main` / `rust-lr1121` (es del driver, sirve a los
  dos nodos; hoy vive solo en `esp32-devkit-node`).
- Restaurar el firmware real del nodo 1 (quedó con el diagnóstico `rawver_c3`).
- Decidir sobre los bins de diagnóstico `node/src/bin/rawver*.rs` (herramienta o
  borrar).
- **Firmware RX (paso 3):** hoy los nodos son solo TX. Para una prueba de
  distancia con RSSI/PER hace falta el lado receptor.
- Monitor serial (Web Serial) para operar los nodos en las pruebas — ver notas.
