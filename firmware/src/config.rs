use eeprom;
use crc32;
use smoltcp::wire::IpAddress;

const MAGIC: u8 = 0x54;

struct EepromReader {
    buffer: [u8; eeprom::BLOCK_LEN]
}

impl EepromReader {
    fn new() -> EepromReader {
        EepromReader {
            buffer: [0; eeprom::BLOCK_LEN]
        }
    }

    fn read_payload_block<'a>(&'a mut self, block: u16) -> bool {
        eeprom::read_block(&mut self.buffer, block);

        if self.buffer[0] != MAGIC {
            return false;
        }
        let cksum = self.buffer[0] as u32 | (self.buffer[1] as u32) << 8 |
                   (self.buffer[2] as u32) << 16 | (self.buffer[3] as u32) << 24;
        if crc32::checksum_ieee(&self.buffer[0..self.buffer.len()-4]) != cksum {
            return false;
        }
        true
    }
    
    fn read_payload<'a>(&'a mut self) -> Result<&'a [u8], ()> {
        let mut ok = self.read_payload_block(0);
        if !ok {
            ok = self.read_payload_block(1);
        }
        if ok {
            Ok(&self.buffer[1..self.buffer.len()-4])
        } else {
            Err(())
        }
    }
}

fn write_eeprom_payload(payload: &[u8]) {
    let mut buffer: [u8; eeprom::BLOCK_LEN] = [0; eeprom::BLOCK_LEN];
    buffer[0] = MAGIC;
    buffer[1..payload.len()+1].copy_from_slice(payload);
    let len = buffer.len();
    let cksum = crc32::checksum_ieee(&buffer[0..len-4]);
    buffer[len-4] = cksum as u8;
    buffer[len-3] = (cksum >> 8) as u8;
    buffer[len-2] = (cksum >> 16) as u8;
    buffer[len-1] = (cksum >> 24) as u8;
    eeprom::write_block(&buffer, 0);
    eeprom::write_block(&buffer, 1);
}

pub struct Config {
    pub ip: IpAddress,
}

impl Config {
    pub fn new() -> Config {
        Config {
            ip: IpAddress::v4(192, 168, 69, 1)
        }
    }
    
    pub fn load(&mut self) {
        let mut reader = EepromReader::new();
        let payload = reader.read_payload();
        if payload.is_ok() {
            let payload = payload.unwrap();
            self.ip = IpAddress::v4(payload[0], payload[1], payload[2], payload[3]);
        }
    }
    
    pub fn save(&self) {
        let mut payload: [u8; 4] = [0; 4];
        let ip4 = match self.ip {
            IpAddress::Ipv4(x) => x.0,
            _ => panic!("unsupported network address")
        };
        payload[0..4].copy_from_slice(&ip4);
    }
}
