# Protocolo

Documento de referencia del protocolo: paquete LoRa P2P y servicio BLE/GATT que
consume la app. **A completar** a medida que se implemente el firmware.

## Paquete LoRa (tentativo)

| Campo | Tipo | Nota |
|-------|------|------|
| seq   | u16  | número de secuencia |
| ...   | ...  | payload a definir |
| crc   | u8   | CRC-8/MAXIM |

## Servicio BLE (GATT)

| Característica | Props | Contenido |
|---------------|-------|-----------|
| Paquetes/métricas | notify | seq, RSSI (dBm), SNR (dB), payload |
| Configuración | read/write | freq, SF, BW, CR, potencia TX |

## Parámetros de radio por defecto

`916.8 MHz · SF7 · BW125 · CR 4/5` (AU915 sub-banda 2). Persistidos en NVS del nodo.
