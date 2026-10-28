# p2p-lora-link

Enlace **punto a punto (P2P) sobre LoRa**, con puente **BLE** a una **app móvil**.

## Objetivo técnico

Generar un enlace P2P mediante **LoRa + BLE + app móvil**:

- Dos nodos LoRa (**ESP32-C3 + LR1121**) intercambian paquetes P2P.
- Ambos nodos exponen por **BLE (GATT)** los datos del enlace.
- Una **app móvil** se conecta por BLE, visualiza los paquetes/métricas y
  permite configurar el radio (frecuencia, SF, BW, CR, potencia).

Ambos nodos son transceptores; el patrón inicial es request–response sobre un
canal half-duplex, con evolución natural a P2P bidireccional. La métrica del
enlace es **RSSI + SNR + tasa de paquetes (PER)**.

## Estructura del repo

```
p2p-lora-link/
├── firmware/   # Rust (esp-idf), nodos ESP32-C3 + LR1121
├── app/        # app móvil (Expo / React Native), cliente BLE
└── docs/       # protocolo BLE/paquetes, hardware, notas de la PoC
```

Firmware y app viven juntos a propósito: **comparten el protocolo BLE/GATT**, así
un cambio de protocolo se hace en un solo commit tocando ambos lados.

## Hardware

- 2× **ESP32-C3** (SuperMini).
- 2× **LR1121** (interfaz SPI).
- Antenas para 915 MHz (omni para banco; direccional/Yagi para alcance).
- Pigtails UFL→SMA.

> Límite de entrada de RF del LR1121: **0 dBm**. En banco, bajar la potencia de
> TX y no usar antenas de alta ganancia a corta distancia.

## Firmware (`firmware/`)

- Lenguaje: **Rust** sobre **esp-idf**.
- Target: **ESP32-C3** (`riscv32imc-esp-espidf`).
- Toolchain: `espup install -r` (el `-r` trae el GCC de RISC-V; sin él la build
  del C3 falla), luego `. ~/export-esp.sh`.

## App (`app/`)

- **Expo / React Native**, cliente **BLE** (GATT).
- Pantallas: conectar/escanear dispositivos, monitor de paquetes y métricas,
  configuración del radio (persistida en NVS del nodo).

## Protocolo

Los nodos exponen un servicio GATT con:
- **Paquetes/métricas** (notify): seq, RSSI, SNR, payload.
- **Configuración** (read/write): frecuencia, SF, BW, CR, potencia.

Detalle en [`docs/protocolo.md`](docs/protocolo.md).

## Parámetros de radio por defecto

`916.8 MHz · SF7 · BW125 · CR 4/5` (AU915 sub-banda 2). Configurables desde la app.

## Estado / Kanban

El seguimiento del trabajo (tareas, estado, prioridades) vive en el tablero de
GitHub Projects:

👉 **[Kanban del proyecto](https://github.com/users/Ignacio-Ucedo/projects/2)**


