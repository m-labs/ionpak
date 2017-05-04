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

fn main() {
    hprintln!("Hello, world!");

    cortex_m::interrupt::free(|cs| {
        let systick = tm4c129x::SYST.borrow(cs);
        let sysctl  = tm4c129x::SYSCTL.borrow(cs);
        let gpio_k  = tm4c129x::GPIO_PORTK.borrow(cs);

        // Set up system timer
        systick.set_reload(systick.get_ticks_per_10ms());
        systick.enable_counter();
        systick.enable_interrupt();

        // Set up LED
        sysctl.rcgcgpio.modify(|_, w| w.r9().bit(true));
        while !sysctl.prgpio.read().r9().bit() {}

        gpio_k.dir.write(|w| w.dir().bits(0x10));
        gpio_k.den.write(|w| w.den().bits(0x10));
    });
}

use cortex_m::exception::SysTick;

extern fn sys_tick(ctxt: SysTick) {
    static ELAPSED: Local<Cell<u32>, SysTick> = Local::new(Cell::new(0));
    let elapsed = ELAPSED.borrow(&ctxt);

    elapsed.set(elapsed.get() + 1);

    cortex_m::interrupt::free(|cs| {
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);

        if elapsed.get() % 100 == 0 {
            gpio_k.data.modify(|r, w| w.data().bits(r.data().bits() ^ 0x10));
        }
    })
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
 