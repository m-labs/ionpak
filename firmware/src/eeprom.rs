use cortex_m;
use tm4c129x;

use board;

pub const BLK_COUNT: u16 = 96;   // Number of blocks
pub const BLK_U32_LEN: usize = 16; // Number of words in a block
const PRETRY: u32 = 0x00000004;  // Programming Must Be Retried
const ERETRY: u32 = 0x00000008;  // Erase Must Be Retried

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

pub fn mass_erase() -> bool {
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eedbgme.write(|w| unsafe { w.key().bits(0xE37B).me().bit(true) });
    });
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(true)); // Activate EEPROM reset
        board::delay(16);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(false)); // Dectivate EEPROM reset
        board::delay(16);
        while !sysctl.preeprom.read().r0().bit() {} // Wait for the EEPROM to come out of reset
        board::delay(16);
    });
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        let eesupp2 = eeprom.eesupp.read().bits();
        eesupp2 & (PRETRY | ERETRY) == 0
    })
}

pub fn read_blk(buf: &mut [u32; BLK_U32_LEN], blk: u16) {
    assert!(blk < BLK_COUNT);
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(blk) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
    });
    for i in 0..BLK_U32_LEN {
        cortex_m::interrupt::free(|cs| {
            let eeprom = tm4c129x::EEPROM.borrow(cs);
            buf[i] = eeprom.eerdwrinc.read().bits();
        });
    }
}

pub fn write_blk(buf: &[u32; BLK_U32_LEN], blk: u16) {
    assert!(blk < BLK_COUNT);
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(blk) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
    });
    for i in 0..BLK_U32_LEN {
        cortex_m::interrupt::free(|cs| {
            let eeprom = tm4c129x::EEPROM.borrow(cs);
            eeprom.eerdwrinc.write(|w| unsafe { w.bits(buf[i]) });
        });
        board::delay(16);
        wait_done();
    }
}
