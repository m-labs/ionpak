#![feature(used, const_fn, core_float, asm)]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate tm4c129x;
extern crate smoltcp;

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
mod ethmac;
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
    // Enable the FPU
    unsafe {
        asm!("
            PUSH {R0, R1}
            LDR.W R0, =0xE000ED88
            LDR R1, [R0]
            ORR R1, R1, #(0xF << 20)
            STR R1, [R0]
            DSB
            ISB
            POP {R0, R1}
        ");
    }
    // Beware of the compiler inserting FPU instructions
    // in the prologue of functions before the FPU is enabled!
    main_with_fpu();
}

#[inline(never)]
fn main_with_fpu() {
    board::init();

    cortex_m::interrupt::free(|cs| {
        let nvic = tm4c129x::NVIC.borrow(cs);
        nvic.enable(Interrupt::ADC0SS0);

        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();

        // ZJ-10
        let anode = 165.0;
        let cathode_bias = 50.0;
        let emission = 0.5e-3;

        // ZJ-27
        /*let anode = 225.0;
        let cathode_bias = 25.0;
        let emission = 1.0e-3;*/

        // ZJ-12
        /*let anode = 200.0;
        let cathode_bias = 50.0;
        let emission = 4.0e-3;*/

        // G8130
        /*let anode = 180.0;
        let cathode_bias = 30.0;
        let emission = 4.0e-3;*/

        loop_anode.set_target(anode);
        loop_cathode.set_emission_target(emission);
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
            next_blink = time + 1000;
            board::set_led(1, led_state);
        }

        if time >= next_info {
            let (anode, cathode, electrometer) = cortex_m::interrupt::free(|cs| {
                (LOOP_ANODE.borrow(cs).borrow().get_status(),
                 LOOP_CATHODE.borrow(cs).borrow().get_status(),
                 ELECTROMETER.borrow(cs).borrow().get_status())
            });

            println!("");
            anode.debug_print();
            cathode.debug_print();
            electrometer.debug_print();
            if cathode.fbi.is_some() && electrometer.ic.is_some() {
                let fbi = cathode.fbi.unwrap();
                let ic = electrometer.ic.unwrap();
                let pressure = ic/fbi/18.75154;
                println!("{:.1e} mbar", pressure);
            }

            next_info = next_info + 3000;
        }

        if board::error_latched() {
            match latch_reset_time {
                None => {
                    println!("Protection latched");
                    latch_reset_time = Some(time + 10000);
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
