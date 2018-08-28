use libm::F32Ext;

use board;
use pid;

const PID_PARAMETERS: pid::Parameters = pid::Parameters {
    kp: 0.2,
    ki: 0.05,
    kd: 0.0,
    output_min: 0.0,
    output_max: 225.0,
    integral_min: -700.0,
    integral_max: 700.0
};


pub struct Controller {
    pid: pid::Controller,
    target: f32,
    last_av: Option<f32>
}

#[derive(Clone, Copy)]
pub struct ControllerStatus {
    pub ready: bool,
    pub av: Option<f32>
}

impl Controller {
    pub const fn new() -> Controller {
        Controller {
            pid: pid::Controller::new(PID_PARAMETERS),
            target: 0.0,
            last_av: None
        }
    }

    pub fn adc_input(&mut self, av_sample: u16) {
        let av = av_sample as f32/board::AV_ADC_GAIN;
        self.last_av = Some(av);

        let hv_pwm_duty = self.pid.update(av);
        board::set_hv_pwm(hv_pwm_duty as u16)
    }

    pub fn set_target(&mut self, volts: f32) {
        self.target = volts;
        self.pid.set_target(volts);
    }

    fn ready(&self) -> bool {
        match self.last_av {
            None => false,
            Some(last_av) => (last_av - self.target).abs() < 2.0
        }
    }

    pub fn reset(&mut self) {
        self.pid.reset();
        board::set_hv_pwm(0);
    }

    pub fn get_status(&self) -> ControllerStatus {
        ControllerStatus {
            ready: self.ready(),
            av: self.last_av
        }
    }
}
