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
const HV_PWM: u8 = 0x01;
const FV_PWM: u8 = 0x04;

const PWM_LOAD: u32 = /*pwmclk*/16_000_000 / /*freq*/100_000;

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

fn set_fv_pwm(duty: u8) {

}

fn set_hv_pwm(duty: u8) {

}

fn main() {
    hprintln!("Hello, world!");

    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);

        // Set up system timer
        let systick = tm4c129x::SYST.borrow(cs);
        systick.set_reload(systick.get_ticks_per_10ms());
        systick.enable_counter();
        systick.enable_interrupt();

        // Bring up GPIO port F and K
        sysctl.rcgcgpio.modify(|_, w| w.r5().bit(true));
        while !sysctl.prgpio.read().r5().bit() {}
        sysctl.rcgcgpio.modify(|_, w| w.r9().bit(true));
        while !sysctl.prgpio.read().r9().bit() {}

        // Set up LEDs
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);
        gpio_k.dir.write(|w| w.dir().bits(LED1|LED2));
        gpio_k.den.write(|w| w.den().bits(LED1|LED2));

        // Set up PWMs
        let gpio_f = tm4c129x::GPIO_PORTF_AHB.borrow(cs);
        gpio_f.dir.write(|w| w.dir().bits(HV_PWM|FV_PWM));
        gpio_f.den.write(|w| w.den().bits(HV_PWM|FV_PWM));
        gpio_f.afsel.write(|w| w.afsel().bits(HV_PWM|FV_PWM));
        gpio_f.pctl.write(|w| unsafe { w.pmc0().bits(6).pmc2().bits(6) });

        sysctl.rcgcpwm.modify(|_, w| w.r0().bit(true));
        while !sysctl.prpwm.read().r0().bit() {}

        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._0_gena.write(|w| w.actload().zero().actcmpad().one());
        pwm0._0_load.write(|w| unsafe { w.bits(PWM_LOAD) });
        pwm0._0_cmpa.write(|w| unsafe { w.bits(PWM_LOAD / 64) });
        pwm0._0_ctl.write(|w| w.enable().bit(true));
        pwm0._1_gena.write(|w| w.actload().zero().actcmpad().one());
        pwm0._1_load.write(|w| unsafe { w.bits(PWM_LOAD) });
        pwm0._1_cmpa.write(|w| unsafe { w.bits(PWM_LOAD / 32) });
        pwm0._1_ctl.write(|w| w.enable().bit(true));
        pwm0.enable.write(|w| w.pwm0en().bit(true).pwm2en().bit(true));
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
 