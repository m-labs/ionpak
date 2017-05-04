#![feature(used, const_fn)]
#![no_std]

#[macro_use]
extern crate cortex_m;
extern crate cortex_m_rt;
extern crate tm4c129x;

use core::cell::Cell;
use cortex_m::ctxt::Local;
use cortex_m::exception::Handlers as ExceptionHandlers;
use tm4c129x::interrupt::Handlers as InterruptHandlers;

const LED1: u8 = 0x10;
const LED2: u8 = 0x40;

fn set_led(nr: u8, state: bool) {
    cortex_m::interrupt::free(|cs| {
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);
        if state {
            gpio_k.data.modify(|r, w| w.data().bits(r.data().bits() | nr))
        } else {
            gpio_k.data.modify(|r, w| w.data().bits(r.data().bits() & !nr))
        }
    });
}

fn main() {
    hprintln!("Hello, world!");

    cortex_m::interrupt::free(|cs| {
        let systick = tm4c129x::SYST.borrow(cs);
        let sysctl  = tm4c129x::SYSCTL.borrow(cs);
        let gpio_k  = tm4c129x::GPIO_PORTK.borrow(cs);

        // Bring up GPIO port K
        sysctl.rcgcgpio.modify(|_, w| w.r9().bit(true));
        while !sysctl.prgpio.read().r9().bit() {}

        // Set up LEDs
        gpio_k.dir.write(|w| w.dir().bits(LED1|LED2));
        gpio_k.den.write(|w| w.den().bits(LED1|LED2));

        // Set up system timer
        systick.set_reload(systick.get_ticks_per_10ms());
        systick.enable_counter();
        systick.enable_interrupt();
    });
}

use cortex_m::exception::SysTick;

extern fn sys_tick(ctxt: SysTick) {
    static ELAPSED: Local<Cell<u32>, SysTick> = Local::new(Cell::new(0));
    let elapsed = ELAPSED.borrow(&ctxt);

    elapsed.set(elapsed.get() + 1);
    if elapsed.get() % 100 == 0 {
        set_led(LED1, true);
        set_led(LED2, false);
    }
    if elapsed.get() % 100 == 50 {
        set_led(LED1, false);
        set_led(LED2, true);
    }
}

#[used]
#[link_section = ".rodata.exceptions"]
pub static EXCEPTIONS: ExceptionHandlers = ExceptionHandlers {
    sys_tick: sys_tick,
    ..cortex_m::exception::DEFAULT_HANDLERS
};

#[used]
#[link_section = ".rodata.interrupts"]
pub static INTERRUPTS: InterruptHandlers = InterruptHandlers {
    ..tm4c129x::interrupt::DEFAULT_HANDLERS
};
 