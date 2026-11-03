# Ejemplos
Estos archivos de prueba sirvieron para validar las distintas etapas
del proceso y el funcionamiento de los dispositivos (microcontrolador, módulos
LoRa, conexiones, soldaduras, etc.)
## Características
### Lora XL1278-SMT
>Los colores utilizados en los pines son para referencia personal
> 
> **ESP32S3** | Pines transmitter
>- VCC: Rojo 3V3
>- GND: Negro GND
>- SLC: Amarillo GPIO12
>- MISO: Naranja GPIO13
>- MOSI: Azul GPIO11
>- NSS: Verde GPIO10
>- RESET: Blanco GPIO14
>- DIO0: Blanco GPIO9
>
> **ESP32WROOM** | Pines receiver
>- VCC: Rojo 3V3
>- GND: Negro GND
>- SLC: Amarillo GPIO18
>- MISO: Naranja GPIO19
>- MOSI: Azul GPIO23
>- NSS: Verde GPIO5
>- RESET: Blanco GPIO14
>- DIO0: Blanco GPIO26

<img src="examples/images/esp32s3wroom1278.png" width="480"/>
<img src="examples/images/esp32c31278.png" width="480"/>

## Prueba 1
### Transmitter: Esp32S3 / Receiver: Esp32WROOM
Estos archivos involucran una comunicación mediante un módulo LX1278-SMT usando
Arduino
```
examples/prueba1/
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
examples/prueba2
├── esp32s3-transmitter.go  # Transmisor
examples/prueba1
└── esp32wroom-receiver.ino # Receptor

```
## Prueba 3
### Transmitter: Esp32C3 / Receiver: Esp32C3
En esta parte, se logró el envío de un mensaje de un módulo a otro de forma
parcial, alegando errores en la comunicación
```
Receptor LoRa (ESP32-C3)
RegVersion = 18 (esperado 18)
LoRa init OK, esperando paquetes...
Recibido: UHHello World, this is | RSSI: -164 dBm | SNR: 0 dB
```
También es útil mencionar que hay una reutilización de partes del código,
como los pines o determinadas funciones que pueden abstraerse en otro archivo.

```
examples/prueba3
├── esp32c3-transmitter.go  # Transmisor
└── esp32c3-receiver.go     # Receptor

```