use cortex_m;
use tm4c129x;

use board;

pub const BLOCK_COUNT: u16 = 96;
pub const BLOCK_LEN: usize = 64;

fn wait_done() {
    while cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eedone.read().working().bit()
    }) {};
}

pub fn init() {
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);

        sysctl.rcgceeprom.modify(|_, w| w.r0().bit(true)); // Bring up EEPROM
        board::delay(16);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(true)); // Activate EEPROM reset
        board::delay(16);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(false)); // Dectivate EEPROM reset
        board::delay(16);
        while !sysctl.preeprom.read().r0().bit() {} // Wait for the EEPROM to come out of reset
        board::delay(16);
    });
    wait_done();
}

pub fn read_block(buffer: &mut [u8; BLOCK_LEN], block: u16) {
    assert!(block < BLOCK_COUNT);
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(block) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
        for i in 0..BLOCK_LEN/4 {
            let word = eeprom.eerdwrinc.read().bits();
            buffer[4*i] = word as u8;
            buffer[4*i+1] = (word >> 8) as u8;
            buffer[4*i+2] = (word >> 16) as u8;
            buffer[4*i+3] = (word >> 24) as u8;
        }
    });
}

pub fn write_block(buffer: &[u8; BLOCK_LEN], block: u16) {
    assert!(block < BLOCK_COUNT);
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(block) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
    });
    for i in 0..BLOCK_LEN/4 {
        let word = buffer[4*i] as u32 | (buffer[4*i+1] as u32) << 8 |
                   (buffer[4*i+2] as u32) << 16 | (buffer[4*i+3] as u32) << 24;
        cortex_m::interrupt::free(|cs| {
            let eeprom = tm4c129x::EEPROM.borrow(cs);
            eeprom.eerdwrinc.write(|w| unsafe { w.bits(word) });
        });
        board::delay(16);
        wait_done();
    }
}
