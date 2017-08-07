use core::fmt;

use cortex_m;
use tm4c129x;

use ethmac::delay;

const EEPROM_BLK_COUNT: u16 = 96;          // Number of the blocks

const EEPROM_BLK_U32_LEN: u16 = 16;        // Number of the words in a block

const EEPROM_PRETRY: u32 =     0x00000004; // Programming Must Be Retried
const EEPROM_ERETRY: u32 =     0x00000008; // Erase Must Be Retried

fn wait_done() {
    unsafe {
        let eeprom = tm4c129x::EEPROM.get();
         // Make sure the EEPROM is idle
        while (*eeprom).eedone.read().working().bit() {};
    }
}

pub fn init() -> u32 {
    let status: u32 = 0;
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);
        let eeprom = tm4c129x::EEPROM.borrow(cs);

        sysctl.rcgceeprom.modify(|_, w| w.r0().bit(true)); // Bring up EEPROM
        delay(16);
        let eesupp1 = eeprom.eesupp.read().bits();
        if 0 != eesupp1 & (EEPROM_PRETRY | EEPROM_ERETRY) {
            println!("eesupp1:{}", eesupp1)
        }
        sysctl.sreeprom.modify(|_, w| w.r0().bit(true)); // Activate EEPROM reset
        delay(16);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(false)); // Dectivate EEPROM reset
        delay(16);
        while !sysctl.preeprom.read().r0().bit() {} // Wait for the EEPROM to come out of reset
        delay(16);
        wait_done();
        let eesupp2 = eeprom.eesupp.read().bits();
        if 0 != eesupp2 & (EEPROM_PRETRY | EEPROM_ERETRY) {
            println!("eesupp2:{}", eesupp2)
        }
        let eesize_blkcnt = eeprom.eesize.read().blkcnt().bits();
        println!("EESIZE_BLK:{}", eesize_blkcnt)
    });
    status
}

pub fn mass_erase() {
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eedbgme.write(|w| unsafe { w.key().bits(0xE37B).me().bit(true) });
    });
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(true)); // Activate EEPROM reset
        delay(16);
        sysctl.sreeprom.modify(|_, w| w.r0().bit(false)); // Dectivate EEPROM reset
        delay(16);
        while !sysctl.preeprom.read().r0().bit() {} // Wait for the EEPROM to come out of reset
        delay(16);
    });
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        let eesupp2 = eeprom.eesupp.read().bits();
        if 0 != eesupp2 & (EEPROM_PRETRY | EEPROM_ERETRY) {
            println!("eesupp2:{}", eesupp2)
        } else {
            println!("erase_ok");
        }
    });
}

pub fn read_blk(buf: &mut [u32; 16], blk: u16, verify: bool) -> u8 {
    let mut result : u8 = 0;
    assert!(blk < EEPROM_BLK_COUNT);
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(blk) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
    });
    for i in 0..EEPROM_BLK_U32_LEN {
        cortex_m::interrupt::free(|cs| {
            let eeprom = tm4c129x::EEPROM.borrow(cs);
            if verify {
                if buf[i as usize] != eeprom.eerdwrinc.read().bits() {
                    result += 1;
                }
            } else {
                buf[i as usize] = eeprom.eerdwrinc.read().bits();
            }
        });
    }
    result
}

pub fn write_blk(buf: &[u32; 16], blk: u16) -> u8 {
    assert!(blk < EEPROM_BLK_COUNT);
    wait_done();
    cortex_m::interrupt::free(|cs| {
        let eeprom = tm4c129x::EEPROM.borrow(cs);
        eeprom.eeblock.write(|w| unsafe { w.block().bits(blk) });
        eeprom.eeoffset.write(|w| unsafe { w.offset().bits(0) });
    });
    for i in 0..EEPROM_BLK_U32_LEN {
        cortex_m::interrupt::free(|cs| {
            let eeprom = tm4c129x::EEPROM.borrow(cs);
            eeprom.eerdwrinc.write(|w| unsafe { w.bits(buf[i as usize]) });
        });
        delay(16);
        wait_done();
    }
    0
}