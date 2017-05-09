#![feature(used, const_fn, core_float)]
#![no_std]

#[macro_use]
extern crate cortex_m;
extern crate cortex_m_rt;
extern crate tm4c129x;

use core::cell::{Cell, RefCell};
use core::fmt;
use cortex_m::ctxt::Local;
use cortex_m::exception::Handlers as ExceptionHandlers;
use cortex_m::interrupt::Mutex;
use tm4c129x::interrupt::Interrupt;
use tm4c129x::interrupt::Handlers as InterruptHandlers;

mod board;
mod pid;
mod loop_anode;
mod loop_cathode;


static LOOP_ANODE: Mutex<RefCell<loop_anode::Controller>> = Mutex::new(RefCell::new(
    loop_anode::Controller::new()));

static LOOP_CATHODE: Mutex<RefCell<loop_cathode::Controller>> = Mutex::new(RefCell::new(
    loop_cathode::Controller::new()));


pub struct UART0;

impl fmt::Write for UART0 {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for c in s.bytes() {
            unsafe {
                let uart_0 = tm4c129x::UART0.get();
                while (*uart_0).fr.read().txff().bit() {}
                (*uart_0).dr.write(|w| w.data().bits(c))
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        write!($crate::UART0, $($arg)*).unwrap()
    })
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}


fn main() {
    board::init();

    cortex_m::interrupt::free(|cs| {
        // Enable FPU
        let scb = tm4c129x::SCB.borrow(cs);
        scb.enable_fpu();

        let nvic = tm4c129x::NVIC.borrow(cs);
        nvic.enable(Interrupt::ADC0SS0);

        board::set_emission_range(board::EmissionRange::High);
        let bias = 15.0;
        LOOP_ANODE.borrow(cs).borrow_mut().set_target(70.0+bias);
        LOOP_CATHODE.borrow(cs).borrow_mut().set_bias_target(bias);
        //board::set_fv_pwm(10);
    });

    println!("ready");

    loop {
        board::process_errors();
    }
}

use tm4c129x::interrupt::ADC0SS0;
extern fn adc0_ss0(ctxt: ADC0SS0) {
    static ELAPSED: Local<Cell<u32>, ADC0SS0> = Local::new(Cell::new(0));
    let elapsed = ELAPSED.borrow(&ctxt);

    cortex_m::interrupt::free(|cs| {
        let adc0 = tm4c129x::ADC0.borrow(cs);
        if adc0.ostat.read().ov0().bit() {
            panic!("ADC FIFO overflowed")
        }
        adc0.isc.write(|w| w.in0().bit(true));

        let _ic_sample = adc0.ssfifo0.read().data().bits();
        let fbi_sample = adc0.ssfifo0.read().data().bits();
        let fv_sample  = adc0.ssfifo0.read().data().bits();
        let fd_sample  = adc0.ssfifo0.read().data().bits();
        let av_sample  = adc0.ssfifo0.read().data().bits();
        let fbv_sample = adc0.ssfifo0.read().data().bits();

        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();
        loop_anode.adc_input(av_sample);
        loop_cathode.adc_input(fbi_sample, fd_sample, fv_sample, fbv_sample);

        elapsed.set(elapsed.get() + 1);
        if elapsed.get() % 100 == 0 {
            board::set_led(1, true);
            board::set_led(2, false);
        }
        if elapsed.get() % 100 == 50 {
            board::set_led(1, false);
            board::set_led(2, true);
        }
    })
}

#[used]
#[link_section = ".rodata.exceptions"]
pub static EXCEPTIONS: ExceptionHandlers = ExceptionHandlers {
    ..cortex_m::exception::DEFAULT_HANDLERS
};

#[used]
#[link_section = ".rodata.interrupts"]
pub static INTERRUPTS: InterruptHandlers = InterruptHandlers {
    ADC0SS0: adc0_ss0,
    ..tm4c129x::interrupt::DEFAULT_HANDLERS
};
