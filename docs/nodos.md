# Inventario de nodos

Tres nodos LR1121, dos familias de MCU. **La placa define target + pinout +
consola**, y eso vive por-rama en `firmware/` (no hay un único build para todos).

| # | MCU | Rol | Puerto | Consola | Rama | BLE |
|---|-----|-----|--------|---------|------|-----|
| **1** | ESP32-C3 SuperMini | TX | `/dev/ttyACM0` | USB-Serial/JTAG nativo | `rust-lr1121` | — |
| **2** | ESP32 clásico (devkit) | TX | `/dev/ttyUSB0` | UART0 vía puente USB-UART | `esp32-devkit-node` | `LORA-NODE-02` |
| **3** | ESP32-C3 SuperMini | **RX** (paso 3) ✅ | `/dev/ttyACM0` | USB-Serial/JTAG nativo | `rust-lr1121` (bin `rx`) | — |

> **Enlace validado (2026-11-12):** nodo 2 (TX) → nodo 3 (RX) a 4 m, **0% PER**,
> rssi ~-93 dBm, snr ~14 dB. Costó 4 bugs del camino de RX (nunca corrido en HW);
> detalle en la memoria `lr1121-rx-cmderr-quirk`. Antenas 868 usadas en 916 → ~30 dB
> de margen; para rango real conseguir antenas 915.

## Pinout LR1121 por familia

| Señal | C3 (nodos 1 y 3) | Clásico (nodo 2) |
|-------|------------------|------------------|
| CS    | GPIO7  | GPIO4  |
| CLK   | GPIO4  | GPIO18 |
| MOSI  | GPIO6  | GPIO23 |
| MISO  | GPIO5  | GPIO19 |
| RESET | GPIO3  | GPIO21 |
| BUSY  | GPIO10 | GPIO22 |

- **Clásico:** evitar GPIO6-11 (flash) y GPIO1/3 (UART0 consola).
- **C3 SuperMini:** la consola es USB-Serial/JTAG nativo; hace falta
  `CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=y` en `sdkconfig.defaults` o no se ve nada
  en `espflash --monitor`. El clásico NO lleva ese flag (rompería su consola UART0).

## Radio (los tres deben coincidir o no enganchan)

916.8 MHz · SF7 · BW125 · CR4/5 · CRC on · IQ estándar · potencia 14 dBm.
Son los `DEFAULT_*` del driver `lr1121`.

## Estado y cabos sueltos

- **Nodo 2** es el único con firmware "real" corriendo hoy (TX + control BLE).
- **Nodo 1** quedó con el binario de diagnóstico `rawver_c3` flasheado durante la
  caza del bug de ABI — falta restaurarle su FW de TX (build C3 → ttyACM0).
- **Nodo 3** recibe OK (bin `rx` en `rust-lr1121`, pinout C3): mide seq/RSSI/SNR/PER
  por serial, se lee con `tools/serial-monitor`. Sin ACK ni BLE todavía (paso
  siguiente: ACK downlink con `encode_ack` + `transmit_ack_and_return_rx`).
- **Fixes del driver sin consolidar:** el bring-up del RX agregó varios fixes a
  `lr1121/lib.rs` en `rust-lr1121` (CMD_ERR espúreo, set_rx RTC-step, HP PA, y el
  FFI de `read_buffer8` con `offset`) — falta llevarlos a `esp32-devkit-node`/main.
  El fix del **HP PA** del TX ya está commiteado en `esp32-devkit-node` (nodo 2).
  El fix de ABI del `GetVersion` idem (vive en ambas ramas).

## Gotchas de flasheo

- Clásico: a veces falla el primer `espflash flash` (auto-reset); reintentar o
  mantener BOOT apretado.
- No dejar un nodo alimentado por el USB de la PC durante una suspensión: así se
  perdió el primer C3 (latch-up).
- El puerto es exclusivo: cerrar la pestaña del monitor serial antes de flashear.

## Gotchas de alimentación (pruebas de campo)

- **Powerbank: cuidado con el auto-apagado por baja carga.** El TX en powerbank
  puede no encender o reiniciarse según el puerto (1 A vs 2 A): no es capacidad
  (tira <200 mA), es que el powerbank corta el puerto si el consumo cae bajo
  ~50-100 mA. Usar un puerto/powerbank que no se auto-apague ("low current" /
  "always-on"). Síntoma en el RX: `seq` salta a números bajos y el PER se va a
  99.9% hasta resetear con `r`.
