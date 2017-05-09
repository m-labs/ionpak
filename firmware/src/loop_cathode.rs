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
    pid: pid::Controller,
    fbv_target: f32,
    last_fv: Option<f32>,
    last_fbv: Option<f32>
}


impl Controller {
    pub const fn new() -> Controller {
        Controller {
            pid: pid::Controller::new(PID_PARAMETERS),
            fbv_target: 0.0,
            last_fv: None,
            last_fbv: None,
        }
    }

    pub fn adc_input(&mut self, _fbi_sample: u16, _fd_sample: u16, fv_sample: u16, fbv_sample: u16) {
        self.last_fbv = Some(fbv_sample as f32/board::FBV_ADC_GAIN);
        self.last_fv = Some(fv_sample as f32/board::FV_ADC_GAIN);
    }

    pub fn set_bias_target(&mut self, volts: f32) {
        self.fbv_target = volts;
        board::set_fbv_pwm((volts/board::FBV_PWM_GAIN) as u16);
    }

    pub fn emission_ready(&self) -> bool {
        false
    }

    pub fn bias_ready(&self) -> bool {
        match self.last_fbv {
            None => false,
            Some(last_fbv) => (self.fbv_target - last_fbv).abs() < 1.0
        }
    }

    pub fn ready(&self) -> bool {
        self.emission_ready() & self.bias_ready()
    }
}
