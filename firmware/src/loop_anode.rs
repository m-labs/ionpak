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
    pid: pid::Controller
}


impl Controller {
    pub const fn new() -> Controller {
        Controller {
            pid: pid::Controller::new(PID_PARAMETERS)
        }
    }

    pub fn adc_input(&mut self, av_sample: u16) {
        let pid_out = self.pid.update(av_sample as f32);
        board::set_hv_pwm(pid_out as u16)
    }

    pub fn set_target(&mut self, volts: f32) {
        self.pid.target = volts*board::AV_ADC_GAIN;
    }

    pub fn ready(&self) -> bool {
        self.pid.is_within(1.0*board::AV_ADC_GAIN)
    }

    pub fn reset(&mut self) {
        self.pid.reset();
    }

    pub fn debug_print(&self) {
        println!("anode ready: {}", self.ready());
        if self.pid.last_input.is_some() {
            println!("voltage: {}V", self.pid.last_input.unwrap()/board::AV_ADC_GAIN);
        }
    }
}
