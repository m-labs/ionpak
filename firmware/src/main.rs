#![feature(used, const_fn, core_float)]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate tm4c129x;

use core::cell::{Cell, RefCell};
use core::fmt;
use cortex_m::exception::Handlers as ExceptionHandlers;
use cortex_m::interrupt::Mutex;
use tm4c129x::interrupt::Interrupt;
use tm4c129x::interrupt::Handlers as InterruptHandlers;

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

#[macro_use]
mod board;
mod pid;
mod loop_anode;
mod loop_cathode;
mod electrometer;

static TIME: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));

fn get_time() -> u64 {
    cortex_m::interrupt::free(|cs| {
        TIME.borrow(cs).get()
    })
}

static LOOP_ANODE: Mutex<RefCell<loop_anode::Controller>> = Mutex::new(RefCell::new(
    loop_anode::Controller::new()));

static LOOP_CATHODE: Mutex<RefCell<loop_cathode::Controller>> = Mutex::new(RefCell::new(
    loop_cathode::Controller::new()));

static ELECTROMETER: Mutex<RefCell<electrometer::Electrometer>> = Mutex::new(RefCell::new(
    electrometer::Electrometer::new()));


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

fn main() {
    board::init();

    cortex_m::interrupt::free(|cs| {
        // Enable FPU
        let scb = tm4c129x::SCB.borrow(cs);
        scb.enable_fpu();

        let nvic = tm4c129x::NVIC.borrow(cs);
        nvic.enable(Interrupt::ADC0SS0);

        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();
        let anode_cathode = 20.0;
        let cathode_bias = 12.0;
        loop_anode.set_target(anode_cathode+cathode_bias);
        loop_cathode.set_emission_target(anode_cathode/10000.0);
        loop_cathode.set_bias_target(cathode_bias);
    });

    println!(r#"
  _                         _
 (_)                       | |
  _  ___  _ __  _ __   __ _| |
 | |/ _ \| '_ \| '_ \ / _` | |/ /
 | | (_) | | | | |_) | (_| |   <
 |_|\___/|_| |_| .__/ \__,_|_|\_\
               | |
               |_|
Ready."#);

    let mut next_blink = 0;
    let mut next_info = 0;
    let mut led_state = true;
    let mut latch_reset_time = None;
    loop {
        board::process_errors();

        let time = get_time();

        if time > next_blink {
            led_state = !led_state;
            next_blink = time + 100;
            board::set_led(1, led_state);
        }

        if time > next_info {
            // FIXME: done in ISR now because of FPU snafu
            /*cortex_m::interrupt::free(|cs| {
                LOOP_CATHODE.borrow(cs).borrow().debug_print();
            });*/
            next_info = next_info + 300;
        }

        if board::error_latched() {
            match latch_reset_time {
                None => {
                    println!("Protection latched");
                    latch_reset_time = Some(time + 1000);
                }
                Some(t) => if time > t {
                    latch_reset_time = None;
                    cortex_m::interrupt::free(|cs| {
                        // reset PID loops as they have accumulated large errors
                        // while the protection was active, which would cause
                        // unnecessary overshoots.
                        LOOP_ANODE.borrow(cs).borrow_mut().reset();
                        LOOP_CATHODE.borrow(cs).borrow_mut().reset();
                        board::reset_error();
                    });
                    println!("Protection reset");
                }
            }
        }
    }
}

use tm4c129x::interrupt::ADC0SS0;
extern fn adc0_ss0(_ctxt: ADC0SS0) {
    cortex_m::interrupt::free(|cs| {
        let adc0 = tm4c129x::ADC0.borrow(cs);
        if adc0.ostat.read().ov0().bit() {
            panic!("ADC FIFO overflowed")
        }
        adc0.isc.write(|w| w.in0().bit(true));

        let ic_sample  = adc0.ssfifo0.read().data().bits();
        let fbi_sample = adc0.ssfifo0.read().data().bits();
        let fv_sample  = adc0.ssfifo0.read().data().bits();
        let fd_sample  = adc0.ssfifo0.read().data().bits();
        let av_sample  = adc0.ssfifo0.read().data().bits();
        let fbv_sample = adc0.ssfifo0.read().data().bits();

        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();
        let mut electrometer = ELECTROMETER.borrow(cs).borrow_mut();
        loop_anode.adc_input(av_sample);
        loop_cathode.adc_input(fbi_sample, fd_sample, fv_sample, fbv_sample);
        electrometer.adc_input(ic_sample);

        let time = TIME.borrow(cs);
        time.set(time.get() + 1);

        if time.get() % 300 == 0 {
            println!("");
            loop_anode.get_status().debug_print();
            loop_cathode.get_status().debug_print();
            electrometer.get_status().debug_print();
        }
    });
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
