use core::num::Float;

use board;
use pid;

const PID_PARAMETERS: pid::Parameters = pid::Parameters {
    kp: 0.004,
    ki: 0.002,
    kd: 0.0,
    output_min: 0.0,
    output_max: 30.0,
    integral_min: -5000.0,
    integral_max: 5000.0
};

pub struct Controller {
    fbi_target: f32,
    fbi_range: board::EmissionRange,
    fbi_buffer: [f32; 16],
    fbi_buffer_count: usize,
    last_fbi: Option<f32>,
    pid: pid::Controller,

    last_fv: Option<f32>,

    fbv_target: f32,
    last_fbv: Option<f32>
}


impl Controller {
    pub const fn new() -> Controller {
        Controller {
            fbi_target: 0.0,
            fbi_range: board::EmissionRange::Low,
            fbi_buffer: [0.0; 16],
            fbi_buffer_count: 0,
            last_fbi: None,
            pid: pid::Controller::new(PID_PARAMETERS),

            last_fv: None,

            fbv_target: 0.0,
            last_fbv: None,
        }
    }

    pub fn adc_input(&mut self, fbi_sample: u16, fd_sample: u16, fv_sample: u16, fbv_sample: u16) {
        let fbi_voltage = ((fbi_sample as f32) - board::FBI_ADC_OFFSET)/board::FBI_ADC_GAIN;
        let fbi_r225 = fbi_voltage/board::FBI_R225;
        let fbi = match self.fbi_range {
            board::EmissionRange::Low => fbi_r225,
            board::EmissionRange::Med => {
                let fd_voltage = ((fd_sample as f32) - board::FD_ADC_OFFSET)/board::FD_ADC_GAIN;
                fbi_r225 + (fbi_voltage - fd_voltage)/board::FBI_R223
            },
            board::EmissionRange::High => {
                let fd_voltage = 0.8;
                fbi_r225 + (fbi_voltage - fd_voltage)/board::FBI_R224
            }
        };
        self.fbi_buffer[self.fbi_buffer_count] = fbi;
        self.fbi_buffer_count += 1;
        if self.fbi_buffer_count == self.fbi_buffer.len() {
            let mut fbi_avg: f32 = 0.0;
            for fbi in self.fbi_buffer.iter() {
                fbi_avg += *fbi;
            }
            self.last_fbi = Some(fbi_avg/(self.fbi_buffer.len() as f32));
            self.fbi_buffer_count = 0;
        }

        self.last_fv = Some(fv_sample as f32/board::FV_ADC_GAIN);
        self.last_fbv = Some(fbv_sample as f32/board::FBV_ADC_GAIN);
    }

    pub fn set_emission_target(&mut self, amperes: f32) {
        self.fbi_target = amperes;
        self.fbi_range = board::EmissionRange::Low;
        if amperes > 120e-6 {
            self.fbi_range = board::EmissionRange::Med;
        }
        if amperes > 8e-3 {
            self.fbi_range = board::EmissionRange::High;
        }
        board::set_emission_range(self.fbi_range);
    }

    pub fn set_bias_target(&mut self, volts: f32) {
        self.fbv_target = volts;
        board::set_fbv_pwm((volts/board::FBV_PWM_GAIN) as u16);
    }

    pub fn emission_ready(&self) -> bool {
        match self.last_fbi {
            None => false,
            Some(last_fbi) => (self.fbi_target - last_fbi).abs()/self.fbi_target < 0.02
        }
    }

    pub fn bias_ready(&self) -> bool {
        match self.last_fbv {
            None => false,
            Some(last_fbv) => (self.fbv_target - last_fbv).abs() < 1.0
        }
    }

    pub fn ready(&self) -> bool {
        hprintln!("emission current: {}mA", 1000.0*self.last_fbi.unwrap());
        hprintln!("filament voltage: {}V", self.last_fv.unwrap());
        hprintln!("bias voltage: {}V", self.last_fbv.unwrap());
        self.emission_ready() & self.bias_ready()
    }
}
