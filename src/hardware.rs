// src/hardware.rs — Comunicação serial (Arduino, ESP32, USB-serial)
//
// Funciona em Linux (/dev/ttyACM0, /dev/ttyUSB0) e Windows (COM3, COM4).

use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::Duration;

pub struct SerialDevice {
    port: Box<dyn SerialPort>,
    pub port_name: String,
}

impl SerialDevice {
    /// Lista portas seriais disponiveis.
    /// Retorna Vec<(nome, descricao)> — ex: [("/dev/ttyACM0", "Arduino Uno")]
    pub fn list_ports() -> Vec<(String, String)> {
        match serialport::available_ports() {
            Ok(ports) => ports.into_iter()
                .filter_map(|p| {
                    match &p.port_type {
                        serialport::SerialPortType::UsbPort(usb) => {
                            let desc = format!(
                                "{} ({:04x}:{:04x})",
                                usb.product.clone().unwrap_or_else(|| "USB Serial".into()),
                                usb.vid,
                                usb.pid
                            );
                            Some((p.port_name, desc))
                        }
                        serialport::SerialPortType::BluetoothPort => {
                            Some((p.port_name, "Bluetooth Serial".to_string()))
                        }
                        _ => None,
                    }
                })
                .collect(),
            Err(e) => {
                eprintln!("[serial] erro ao listar portas: {}", e);
                Vec::new()
            }
        }
    }

    /// Abre uma porta serial.
    /// Ex: open("/dev/ttyACM0", 9600) no Linux
    ///     open("COM3", 9600) no Windows
    pub fn open(port_name: &str, baud_rate: u32) -> Result<Self, String> {
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(500))
            .open()
            .map_err(|e| format!("falha ao abrir '{}': {}", port_name, e))?;

        Ok(Self {
            port,
            port_name: port_name.to_string(),
        })
    }

    /// Envia uma linha de texto (adiciona '\n' automaticamente).
    pub fn send_line(&mut self, msg: &str) -> Result<(), String> {
        let data = format!("{}\n", msg);
        self.port
            .write_all(data.as_bytes())
            .map_err(|e| format!("erro ao escrever: {}", e))?;
        self.port
            .flush()
            .map_err(|e| format!("erro no flush: {}", e))?;
        Ok(())
    }

    /// Le uma linha (ate '\n') com timeout.
    pub fn read_line(&mut self) -> Result<String, String> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            match self.port.read(&mut byte) {
                Ok(1) => {
                    if byte[0] == b'\n' {
                        // Remove '\r' se existir (Windows)
                        if buf.last() == Some(&b'\r') {
                            buf.pop();
                        }
                        break;
                    }
                    buf.push(byte[0]);
                }
                Ok(_) => continue,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    return Err("timeout na leitura".to_string());
                }
                Err(e) => return Err(format!("erro ao ler: {}", e)),
            }
        }

        String::from_utf8(buf).map_err(|e| format!("UTF-8 invalido: {}", e))
    }

    /// Le tudo que estiver disponivel (nao bloqueia).
    pub fn read_available(&mut self) -> Result<String, String> {
        let n = self.port.bytes_to_read().unwrap_or(0) as usize;
        if n == 0 {
            return Ok(String::new());
        }

        let mut buf = vec![0u8; n];
        self.port
            .read_exact(&mut buf)
            .map_err(|e| format!("erro ao ler: {}", e))?;

        String::from_utf8(buf).map_err(|e| format!("UTF-8 invalido: {}", e))
    }

    /// Fecha a porta (drop explicito).
    pub fn close(self) {
        // Drop automatico fecha
        drop(self);
    }
}
