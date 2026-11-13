# Mediciones de enlace

Logs crudos de pruebas del enlace LoRa (banco y rango). Son **datos**, por eso se
versionan (el `.gitignore` ignora `*.log` en general, con excepción para esta
carpeta).

## Cómo se capturan

Con el monitor serial (`tools/serial-monitor/index.html`) conectado al **nodo RX**:
botón **"Guardar .log"**. Cada línea trae timestamp del navegador + la línea del
firmware.

## Convención de nombre

`<rol+ubicación>_<distancia>_<condiciones>.log` — descriptivo, para saber qué es
sin abrirlo. Ej.: `rx_interior_tx_exterior_5m_pared_intermedia.log`.
Sugerencia: prefijar la fecha (`2026-11-13_…`) para que ordenen cronológicamente.

## Cómo leer

- `rx seq=N rssi=-XXdBm snr=YYdB | rcv=.. lost=.. per=..%` → un paquete recibido.
  `rssi` en dBm (más negativo = más débil; sensibilidad ~-123 dBm). `per` = % de
  paquetes perdidos. `0% PER` = enlace limpio.
- `rx: vivo @ …` → heartbeat cada 5 s (sigue vivo aunque no lleguen paquetes);
  trae `rssi_pico` de la ventana y flags `visto: pre/hdr/crc/rxDone`.
- `» r` → se envió el comando de **reset de contadores** (para medir PER por
  distancia sin arrastrar las pérdidas de los puntos anteriores).

## Setup de referencia

Radio: 916.8 MHz · SF7 · BW125 · CR4/5 · TX +14 dBm (HP PA). Inventario de nodos y
pinouts en [`../docs/nodos.md`](../docs/nodos.md).
