#include <SPI.h>
#include <LoRa.h>

// --- Pines para ESP32WROOM ---
#define LORA_SCK    18
#define LORA_MISO   19
#define LORA_MOSI   23
#define LORA_SS     5
#define LORA_RST    14
#define LORA_DIO0   26

String inString = "";    // string to hold incoming characters
String MyMessage = "";   // Holds the complete message
unsigned long lastPrint = 0;

void setup() {
    Serial.begin(9600);
    delay(1000); // Pausa para que el SX1278 se estabilice

    Serial.println("LoRa Receiver");

    // Inicializar SPI con los pines personalizados
    SPI.begin(LORA_SCK, LORA_MISO, LORA_MOSI, LORA_SS);

    // Configurar los pines del módulo LoRa (con Reset por hardware)
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

    // try to parse packet
    int packetSize = LoRa.parsePacket();
    if (packetSize) {
        // read packet
        while (LoRa.available())
        {
            int inChar = LoRa.read();
            inString += (char)inChar;
            MyMessage = inString;
        }
        inString = "";
        LoRa.packetRssi();
        // Imprimimos solo cuando llega un paquete, para no llenar el monitor
        Serial.print("Recibido: ");
        Serial.println(MyMessage);
    }
}
