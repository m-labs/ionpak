use cortex_m;
use tm4c129x;


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

const FV_ERRN: u8 = 0x01;    // PL0
const FBV_ERRN: u8 = 0x02;   // PL1
const FBI_ERRN: u8 = 0x04;   // PL2
const AV_ERRN: u8 = 0x08;    // PL3
const AI_ERRN: u8 = 0x10;    // PL4
const ERR_LATCHN: u8 = 0x20; // PL5
const ERR_RESN: u8 = 0x01;   // PQ0

const PWM_LOAD: u16 = (/*pwmclk*/120_000_000u32 / /*freq*/100_000) as u16;
const UART_DIV: u32 = (((/*sysclk*/120_000_000 * 8) / /*baud*/115200) + 1) / 2;


pub const AV_ADC_GAIN: f32 = 6.792703150912105;
pub const FV_ADC_GAIN: f32 = 501.83449105726623;
pub const FBI_ADC_GAIN: f32 = 1333.3333333333333;
pub const FBI_ADC_OFFSET: f32 = 96.0;
pub const FD_ADC_GAIN: f32 = 3111.1111111111104;
pub const FD_ADC_OFFSET: f32 = 96.0;
pub const FBV_ADC_GAIN: f32 = 49.13796058269066;
pub const FBV_PWM_GAIN: f32 = 0.07641071428571428;
pub const IC_ADC_GAIN_LOW: f32 = 1333333333333.3333;
pub const IC_ADC_GAIN_MED: f32 = 13201320132.0132;
pub const IC_ADC_GAIN_HIGH: f32 = 133320001.3332;
pub const IC_ADC_OFFSET: f32 = 96.0;

pub const FBI_R223: f32 = 200.0;
pub const FBI_R224: f32 = 39.0;
pub const FBI_R225: f32 = 22000.0;


pub fn set_led(nr: u8, state: bool) {
    let bit = match nr {
        1 => LED1,
        2 => LED2,
        _ => panic!("unknown LED")
    };
    cortex_m::interrupt::free(|cs| {
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);
        if state {
            gpio_k.data.modify(|r, w| w.data().bits(r.data().bits() | bit))
        } else {
            gpio_k.data.modify(|r, w| w.data().bits(r.data().bits() & !bit))
        }
    });
}

pub fn set_hv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._0_cmpa.write(|w| w.compa().bits(duty));
    });
}

pub fn set_fv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._1_cmpa.write(|w| w.compa().bits(duty));
    });
}

pub fn set_fbv_pwm(duty: u16) {
    cortex_m::interrupt::free(|cs| {
        let pwm0 = tm4c129x::PWM0.borrow(cs);
        pwm0._2_cmpa.write(|w| w.compa().bits(duty));
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmissionRange {
    Low,  // 22K
    Med,  // 22K//(200Ω + compensated diode)
    High  // 22K//(39Ω + uncompensated diode)
}

pub fn set_emission_range(range: EmissionRange) {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ElectrometerRange {
    Low,  // 1G
    Med,  // 1G//10M
    High  // 1G//100K
}

pub fn set_electrometer_range(range: ElectrometerRange) {
    cortex_m::interrupt::free(|cs| {
        let gpio_p = tm4c129x::GPIO_PORTP.borrow(cs);
        gpio_p.data.modify(|r, w| {
            let value = r.data().bits() & 0b111100;
            match range {
                ElectrometerRange::Low  => w.data().bits(value | 0b000000),
                ElectrometerRange::Med  => w.data().bits(value | 0b000001),
                ElectrometerRange::High => w.data().bits(value | 0b000010),
            }
        });
    });
}

pub fn reset_error() {
    cortex_m::interrupt::free(|cs| {
        let gpio_q = tm4c129x::GPIO_PORTQ.borrow(cs);
        gpio_q.data.modify(|r, w| w.data().bits(r.data().bits() & !ERR_RESN));
        gpio_q.data.modify(|r, w| w.data().bits(r.data().bits() | ERR_RESN));
    });
}

pub fn error_latched() -> bool {
    cortex_m::interrupt::free(|cs| {
        let gpio_l = tm4c129x::GPIO_PORTL.borrow(cs);
        gpio_l.data.read().bits() as u8 & ERR_LATCHN == 0
    })
}

pub fn process_errors() {
    let errors_n = cortex_m::interrupt::free(|cs| {
        let gpio_l = tm4c129x::GPIO_PORTL.borrow(cs);
        gpio_l.data.read().bits() as u8
    });
    if errors_n & FV_ERRN == 0 {
        println!("Filament overvolt");
    }
    if errors_n & FBV_ERRN == 0 {
        println!("Filament bias overvolt");
    }
    if errors_n & FBI_ERRN == 0 {
        println!("Filament bias overcurrent");
    }
    if errors_n & AV_ERRN == 0 {
        println!("Anode overvolt");
    }
    if errors_n & AI_ERRN == 0 {
        println!("Anode overcurrent");
    }
}

pub fn init() {
    cortex_m::interrupt::free(|cs| {
        let sysctl = tm4c129x::SYSCTL.borrow(cs);

        // Set up main oscillator
        sysctl.moscctl.write(|w| w.noxtal().bit(false));
        sysctl.moscctl.modify(|_, w| w.pwrdn().bit(false).oscrng().bit(true));

        // Prepare flash for the high-freq clk
        sysctl.memtim0.write(|w| unsafe { w.bits(0x01950195u32) });
        sysctl.rsclkcfg.write(|w| unsafe { w.bits(0x80000000u32) });

        // Set up PLL with fVCO=480 MHz
        sysctl.pllfreq1.write(|w| w.q().bits(0).n().bits(4));
        sysctl.pllfreq0.write(|w| w.mint().bits(96).pllpwr().bit(true));
        sysctl.rsclkcfg.modify(|_, w| w.pllsrc().mosc().newfreq().bit(true));
        while !sysctl.pllstat.read().lock().bit() {}

        // Switch to PLL (sysclk=120MHz)
        sysctl.rsclkcfg.write(|w| unsafe { w.bits(0b1_0_0_1_0011_0000_0000000000_0000000011) });

        // Bring up GPIO ports A, D, E, F, G, K, L, P, Q
        sysctl.rcgcgpio.modify(|_, w| {
            w.r0().bit(true)
             .r3().bit(true)
             .r4().bit(true)
             .r5().bit(true)
             .r6().bit(true)
             .r9().bit(true)
             .r10().bit(true)
             .r13().bit(true)
             .r14().bit(true)
        });
        while !sysctl.prgpio.read().r0().bit() {}
        while !sysctl.prgpio.read().r3().bit() {}
        while !sysctl.prgpio.read().r4().bit() {}
        while !sysctl.prgpio.read().r5().bit() {}
        while !sysctl.prgpio.read().r6().bit() {}
        while !sysctl.prgpio.read().r9().bit() {}
        while !sysctl.prgpio.read().r10().bit() {}
        while !sysctl.prgpio.read().r13().bit() {}
        while !sysctl.prgpio.read().r14().bit() {}

        // Set up UART0
        let gpio_a = tm4c129x::GPIO_PORTA_AHB.borrow(cs);
        gpio_a.dir.write(|w| w.dir().bits(0b11));
        gpio_a.den.write(|w| w.den().bits(0b11));
        gpio_a.afsel.write(|w| w.afsel().bits(0b11));
        gpio_a.pctl.write(|w| unsafe { w.pmc0().bits(1).pmc1().bits(1) });

        sysctl.rcgcuart.modify(|_, w| w.r0().bit(true));
        while !sysctl.pruart.read().r0().bit() {}

        let uart_0 = tm4c129x::UART0.borrow(cs);
        uart_0.cc.write(|w| w.cs().sysclk());
        uart_0.ibrd.write(|w| w.divint().bits((UART_DIV / 64) as u16));
        uart_0.fbrd.write(|w| w.divfrac().bits((UART_DIV % 64) as u8));
        uart_0.lcrh.write(|w| w.wlen()._8().fen().bit(true));
        uart_0.ctl.write(|w| w.rxe().bit(true).txe().bit(true).uarten().bit(true));

        // Set up LEDs
        let gpio_k = tm4c129x::GPIO_PORTK.borrow(cs);
        gpio_k.dir.write(|w| w.dir().bits(LED1|LED2));
        gpio_k.den.write(|w| w.den().bits(LED1|LED2));

        // Set up gain and emission range control pins
        let gpio_p = tm4c129x::GPIO_PORTP.borrow(cs);
        gpio_p.dir.write(|w| w.dir().bits(0b111111));
        gpio_p.den.write(|w| w.den().bits(0b111111));
        set_emission_range(EmissionRange::Med);
        set_electrometer_range(ElectrometerRange::Med);

        // Set up error pins
        let gpio_l = tm4c129x::GPIO_PORTL.borrow(cs);
        let gpio_q = tm4c129x::GPIO_PORTQ.borrow(cs);
        gpio_l.pur.write(|w| w.pue().bits(FV_ERRN|FBV_ERRN|FBI_ERRN|AV_ERRN|AI_ERRN));
        gpio_l.den.write(|w| w.den().bits(FV_ERRN|FBV_ERRN|FBI_ERRN|AV_ERRN|AI_ERRN|ERR_LATCHN));
        gpio_q.dir.write(|w| w.dir().bits(ERR_RESN));
        gpio_q.den.write(|w| w.den().bits(ERR_RESN));
        reset_error(); // error latch is an undefined state upon power-up; reset it

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
        // VCO 480 / 15 = 32MHz ADC clock
        adc0.cc.write(|w| w.cs().syspll().clkdiv().bits(15-1));
        adc0.im.write(|w| w.mask0().bit(true));
        adc0.emux.write(|w| w.em0().always());
        adc0.ssmux0.write(|w| {
            w.mux0().bits(0) // IC_ADC
             .mux1().bits(1) // FBI_ADC
             .mux2().bits(2) // FV_ADC
             .mux3().bits(3) // FD_ADC
             .mux4().bits(5) // AV_ADC
             .mux5().bits(6) // FBV_ADC
        });
        adc0.ssctl0.write(|w| w.ie5().bit(true).end5().bit(true));
        adc0.sstsh0.write(|w| {
            w.tsh0()._4()
             .tsh1()._4()
             .tsh2()._4()
             .tsh3()._4()
             .tsh4()._4()
             .tsh5()._4()
        });
        adc0.sac.write(|w| w.avg()._64x());
        adc0.ctl.write(|w| w.vref().bit(true));
        adc0.actss.write(|w| w.asen0().bit(true));
    });
}
