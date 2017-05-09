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

    pub fn adc_input(&mut self, _fbi_sample: u16, _fd_sample: u16, _fv_sample: u16, _fbv_sample: u16) {
    }
}
