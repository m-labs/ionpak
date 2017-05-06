#![feature(used, const_fn, core_float)]
#![no_std]

#[macro_use]
extern crate cortex_m;
extern crate cortex_m_rt;
extern crate tm4c129x;

use core::cell::{Cell, RefCell};
use cortex_m::ctxt::Local;
use cortex_m::exception::Handlers as ExceptionHandlers;
use cortex_m::interrupt::Mutex;
use tm4c129x::interrupt::Interrupt;
use tm4c129x::interrupt::Handlers as InterruptHandlers;

mod pid;


const HV_PID_PARAMETERS: pid::Parameters = pid::Parameters {
    kp: 0.01,
    ki: 0.005,
    kd: 0.0,
    output_min: 0.0,
    output_max: 30.0,
    integral_min: -5000.0,
    integral_max: 5000.0
};

static HV_PID: Mutex<RefCell<pid::Controller>> = Mutex::new(RefCell::new(
    pid::Controller::new(HV_PID_PARAMETERS)));


const LED1: u8 = 0x10; // PF1
const LED2: u8 = 0x40; // PF3

const HV_PWM: u8 = 0x01;  // PF0
const FV_PWM: u8 = 0x04;  // PF2
const FBV_PWM: u8 = 0x01; // PD5

const FD_ADC: u8 = 0x01;  // PE0
const FV_ADC: u8 = 0x02;  // PE1
const FBI_ADC: u8 = 0x04; // PE2
const IC_ADC: u8 = 0x08;  // PE3
const FBV_ADC: u8 = 0x20; // PD5
const AV_ADC: u8 = 0x40;  // PD6

const FV_ERRN: u8 = 0x01;  // PL0
const FBV_ERRN: u8 = 0x02; // PL1
const FBI_ERRN: u8 = 0x04; // PL2
const AV_ERRN: u8 = 0x08;  // PL3
const AI_ERRN: u8 = 0x10;  // PL4

const PWM_LOAD: u16 = (/*pwmclk*/16_000_000u32 / /*freq*/100_000) as u16;
const ADC_TIMER_LOAD: u32 = /*timerclk*/16_000_000 / /*freq*/100;


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

fn set_hv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._0_cmpa.write(|w| w.compa().bits(duty));
    });
}

fn set_fv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._1_cmpa.write(|w| w.compa().bits(duty));
    });
}

fn set_fbv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._2_cmpa.write(|w| w.compa().bits(duty));
    });
}

#[allow(dead_code)]
enum EmissionRange {
    Low,  // 22K
    Med,  // 22K//(200Ω + compensated diode)
    High  // 22K//(39Ω + uncompensated diode)
}

fn set_emission_range(range: EmissionRange) {
    cortex_m::interrupt::free(|cs| {
        let gpio_p = tm4c129x::GPIO_PORTP.borrow(cs);
        gpio_p.data.modify(|r, w| {
            let value = r.data().bits() & 0b100111;
            match range {
                EmissionRange::Low  => w.data().bits(value | 0b000000),
                EmissionRange::Med  => w.data().bits(value | 0b001000),
                EmissionRange::High => w.data().bits(value | 0b010000),
            }
        });
    });
}


fn main() {
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);
        let nvic = tm4c129x::NVIC.borrow(cs);

        // Set up system timer
        let systick = tm4c129x::SYST.borrow(cs);
        systick.set_reload(systick.get_ticks_per_10ms());
        systick.enable_counter();
        systick.enable_interrupt();

        // Bring up GPIO ports D, E, F, G, K, L, P
        sysctl.rcgcgpio.modify(|_, w| {
            w.r3().bit(true)
             .r4().bit(true)
             .r5().bit(true)
             .r6().bit(true)
             .r9().bit(true)
             .r10().bit(true)
             .r13().bit(true)
        });
        while !sysctl.prgpio.read().r3().bit() {}
        while !sysctl.prgpio.read().r4().bit() {}
        while !sysctl.prgpio.read().r5().bit() {}
        while !sysctl.prgpio.read().r6().bit() {}
        while !sysctl.prgpio.read().r9().bit() {}
        while !sysctl.prgpio.read().r10().bit() {}
        while !sysctl.prgpio.read().r13().bit() {}

        // Set up LEDs
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);
        gpio_k.dir.write(|w| w.dir().bits(LED1|LED2));
        gpio_k.den.write(|w| w.den().bits(LED1|LED2));

        // Set up gain and emission range control pins
        let gpio_p = tm4c129x::GPIO_PORTP.borrow(cs);
        gpio_p.dir.write(|w| w.dir().bits(0b111111));
        gpio_p.den.write(|w| w.den().bits(0b111111));

        // Set up error input pins
        let gpio_l = tm4c129x::GPIO_PORTL.borrow(cs);
        gpio_l.pur.write(|w| w.pue().bits(FV_ERRN|FBV_ERRN|FBI_ERRN|AV_ERRN|AI_ERRN));
        gpio_l.den.write(|w| w.den().bits(FV_ERRN|FBV_ERRN|FBI_ERRN|AV_ERRN|AI_ERRN));

        // Set up PWMs
        let gpio_f = tm4c129x::GPIO_PORTF_AHB.borrow(cs);
        gpio_f.dir.write(|w| w.dir().bits(HV_PWM|FV_PWM));
        gpio_f.den.write(|w| w.den().bits(HV_PWM|FV_PWM));
        gpio_f.afsel.write(|w| w.afsel().bits(HV_PWM|FV_PWM));
        gpio_f.pctl.write(|w| unsafe { w.pmc0().bits(6).pmc2().bits(6) });

        let gpio_g = tm4c129x::GPIO_PORTG_AHB.borrow(cs);
        gpio_g.dir.write(|w| w.dir().bits(FBV_PWM));
        gpio_g.den.write(|w| w.den().bits(FBV_PWM));
        gpio_g.afsel.write(|w| w.afsel().bits(FBV_PWM));
        gpio_g.pctl.write(|w| unsafe { w.pmc0().bits(6) });

        sysctl.rcgcpwm.modify(|_, w| w.r0().bit(true));
        while !sysctl.prpwm.read().r0().bit() {}

        let pwm0 = tm4c129x::PWM0.borrow(cs);
        // HV_PWM
        pwm0._0_gena.write(|w| w.actload().zero().actcmpad().one());
        pwm0._0_load.write(|w| w.load().bits(PWM_LOAD));
        pwm0._0_cmpa.write(|w| w.compa().bits(0));
        pwm0._0_ctl.write(|w| w.enable().bit(true));
        // FV_PWM
        pwm0._1_gena.write(|w| w.actload().zero().actcmpad().one());
        pwm0._1_load.write(|w| w.load().bits(PWM_LOAD));
        pwm0._1_cmpa.write(|w| w.compa().bits(0));
        pwm0._1_ctl.write(|w| w.enable().bit(true));
        // FBV_PWM
        pwm0._2_gena.write(|w| w.actload().zero().actcmpad().one());
        pwm0._2_load.write(|w| w.load().bits(PWM_LOAD));
        pwm0._2_cmpa.write(|w| w.compa().bits(0));
        pwm0._2_ctl.write(|w| w.enable().bit(true));
        // Enable all at once
        pwm0.enable.write(|w| {
            w.pwm0en().bit(true)
             .pwm2en().bit(true)
             .pwm4en().bit(true)
        });

        // Set up ADC
        let gpio_d = tm4c129x::GPIO_PORTD_AHB.borrow(cs);
        let gpio_e = tm4c129x::GPIO_PORTE_AHB.borrow(cs);
        gpio_d.afsel.write(|w| w.afsel().bits(FBV_ADC|AV_ADC));
        gpio_d.amsel.write(|w| w.amsel().bits(FBV_ADC|AV_ADC));
        gpio_e.afsel.write(|w| w.afsel().bits(FD_ADC|FV_ADC|FBI_ADC|IC_ADC));
        gpio_e.amsel.write(|w| w.amsel().bits(FD_ADC|FV_ADC|FBI_ADC|IC_ADC));

        sysctl.rcgcadc.modify(|_, w| w.r0().bit(true));
        while !sysctl.pradc.read().r0().bit() {}

        let adc0 = tm4c129x::ADC0.borrow(cs);
        adc0.actss.write(|w| w.asen0().bit(true));
        adc0.im.write(|w| w.mask0().bit(true));
        adc0.emux.write(|w| w.em0().timer());
        adc0.sac.write(|w| w.avg()._64x());
        adc0.ctl.write(|w| w.vref().bit(true));
        adc0.ssmux0.write(|w| {
            w.mux0().bits(0) // IC_ADC
             .mux1().bits(1) // FBI_ADC
             .mux2().bits(2) // FV_ADC
             .mux3().bits(3) // FD_ADC
             .mux4().bits(5) // AV_ADC
             .mux5().bits(6) // FBV_ADC
        });
        adc0.ssctl0.write(|w| w.end5().bit(true));
        adc0.sstsh0.write(|w| {
            w.tsh0()._256()
             .tsh1()._256()
             .tsh2()._256()
             .tsh3()._256()
             .tsh4()._256()
             .tsh5()._256()
        });

        nvic.enable(Interrupt::ADC0SS0);

        // Set up ADC timer
        sysctl.rcgctimer.modify(|_, w| w.r0().bit(true));
        while !sysctl.prtimer.read().r0().bit() {}

        let timer0 = tm4c129x::TIMER0.borrow(cs);
        timer0.cfg.write(|w| w.cfg()._32_bit_timer());
        timer0.tamr.write(|w| w.tamr().period());
        timer0.tailr.write(|w| unsafe { w.bits(ADC_TIMER_LOAD) });
        timer0.adcev.write(|w| w.tatoadcen().bit(true));
        timer0.cc.write(|w| w.altclk().bit(true));
        timer0.ctl.write(|w| w.taen().bit(true));

        set_emission_range(EmissionRange::Med);
        HV_PID.borrow(cs).borrow_mut().set_target(200.0);
        set_fv_pwm(PWM_LOAD/16);
        set_fbv_pwm(PWM_LOAD/8);
    });

    loop {
        cortex_m::interrupt::free(|cs| {
            let gpio_l = tm4c129x::GPIO_PORTL.borrow(cs);
            let errors_n = gpio_l.data.read().bits() as u8;
            if errors_n & FV_ERRN == 0 {
                hprintln!("Filament overvolt");
            }
            if errors_n & FBV_ERRN == 0 {
                hprintln!("Filament bias overvolt");
            }
            if errors_n & FBI_ERRN == 0 {
                hprintln!("Filament bias overcurrent");
            }
            if errors_n & AV_ERRN == 0 {
                hprintln!("Anode overvolt");
            }
            if errors_n & AI_ERRN == 0 {
                hprintln!("Anode overcurrent");
            }
        });
    }
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

use tm4c129x::interrupt::ADC0SS0;
extern fn adc0_ss0(_ctxt: ADC0SS0) {
    cortex_m::interrupt::free(|cs| {
        let adc0 = tm4c129x::ADC0.borrow(cs);
        if adc0.ostat.read().ov0().bit() {
            panic!("ADC FIFO overflowed")
        }

        let _ic_sample  = adc0.ssfifo0.read().data().bits();
        let _fbi_sample = adc0.ssfifo0.read().data().bits();
        let _fv_sample  = adc0.ssfifo0.read().data().bits();
        let _fd_sample  = adc0.ssfifo0.read().data().bits();
        let av_sample   = adc0.ssfifo0.read().data().bits();
        let _fbv_sample = adc0.ssfifo0.read().data().bits();

        let mut hv_pid = HV_PID.borrow(cs).borrow_mut();
        set_hv_pwm(hv_pid.update(av_sample as f32) as u16);
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
    ADC0SS0: adc0_ss0,
    ..tm4c129x::interrupt::DEFAULT_HANDLERS
};
