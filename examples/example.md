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

## Prueba 2
### Transmitter: Esp32S3 / Receiver: Esp32WROOM
Estos archivos involucran una comunicación mediante un módulo LX1278-SMT usando
TinyGo y Arduino. Es relevante para la detección del error de comunicación del
transmisor, dado que se implementa la interfaz SPI por software, ya que la conexión
directa sugerida para cada pin no lograba inicializarse.
```
examples/
├── esp32s3-transmitter.go  # Transmisor
└── esp32wroom-receiver.ino # Receptor

```
