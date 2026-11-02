# Ejemplos
Estos archivos de prueba sirvieron para validar las distintas etapas
del proceso y el funcionamiento de los dispositivos (microcontrolador, módulos
LoRa, conexiones, soldaduras, etc.)
## Características
### Lora XL1278-SMT
>Los colores utilizados en los pines son (a modo de referencia)
>- VCC: Rojo
>- GND: Negro
>- SLC: Amarillo
>- MISO: Naranja
>- MOSI: Azul
>- NSS: Verde
>- RESET: Blanco
>- DIO0: Blanco

## Prueba 1
### Transmitter: Esp32S3 / Receiver: Esp32WROOM
Estos archivos involucran una comunicación mediante un módulo LX1278-SMT usando
Arduino
```
examples/
├── esp32s3-transmitter.ino # Transmisor
└── esp32wroom-receiver.ino # Receptor

```