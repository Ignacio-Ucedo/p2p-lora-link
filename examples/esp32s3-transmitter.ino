#include <SPI.h>
#include <LoRa.h>

// --- Pines para ESP32-S3 (asignación estándar SPI) ---
#define LORA_SCK    12
#define LORA_MISO   13
#define LORA_MOSI   11
#define LORA_SS     10
#define LORA_RST    14
#define LORA_DIO0   9

void setup() {
    Serial.begin(9600);
    delay(1000); // Pausa para que el SX1278 se estabilice tras el encendido

    Serial.println("LoRa Sender (ESP32-S3)");

    // Inicializar SPI con los pines personalizados
    SPI.begin(LORA_SCK, LORA_MISO, LORA_MOSI, LORA_SS);

    // Configurar los pines del módulo LoRa
    LoRa.setPins(LORA_SS, LORA_RST, LORA_DIO0);

    // Bajar la frecuencia SPI para mayor tolerancia al cableado
    LoRa.setSPIFrequency(4000000); // 4 MHz
    // Estos parámetros deben ser idénticos en emisor y receptor
    LoRa.setSpreadingFactor(9);       // Factor de dispersión (SF7-SF12)
    LoRa.setSignalBandwidth(125E3);   // Ancho de banda (125 kHz es común)
    LoRa.setCodingRate4(5);           // Tasa de codificación (4/5)
    LoRa.setSyncWord(0x12);           // Palabra de sincronización (0x12 privada, 0x34 pública)

    if (!LoRa.begin(433E6)) { // Frecuencia 433Mhz
        Serial.println("Starting LoRa failed!");
        while (1);
    }
    Serial.println("LoRa init OK");
}

void loop() {
    String MyMessage = "Hello World";
    LoRa.beginPacket();
    LoRa.print(MyMessage);
    LoRa.endPacket();
    delay(100);
}
