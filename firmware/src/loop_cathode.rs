use core::num::Float;

use board;
use pid;

const FBI_PID_PARAMETERS: pid::Parameters = pid::Parameters {
    kp: 200.0,
    ki: 20.0,
    kd: 10.0,
    output_min: 0.5,
    output_max: 3.1,
    integral_min: -0.1,
    integral_max: 0.1
};

const FV_PID_PARAMETERS: pid::Parameters = pid::Parameters {
    kp: 20.0,
    ki: 1.5,
    kd: 0.0,
    output_min: 0.0,
    output_max: 150.0,
    integral_min: -50.0,
    integral_max: 50.0
};

pub struct Controller {
    fbi_target: f32,
    fbi_range: board::EmissionRange,
    fbi_buffer: [f32; 16],
    fbi_buffer_count: usize,
    last_fbi: Option<f32>,
    fbi_pid: pid::Controller,
    last_fv_target: Option<f32>,

    fv_pid: pid::Controller,
    last_fv: Option<f32>,

    fbv_target: f32,
    last_fbv: Option<f32>
}

#[derive(Clone, Copy)]
pub struct ControllerStatus {
    pub ready: bool,
    pub fbi: Option<f32>,
    pub fv_target: Option<f32>,
    pub fv: Option<f32>,
    pub fbv: Option<f32>
}

impl Controller {
    pub const fn new() -> Controller {
        Controller {
            fbi_target: 0.0,
            fbi_range: board::EmissionRange::Med,
            fbi_buffer: [0.0; 16],
            fbi_buffer_count: 0,
            last_fbi: None,
            fbi_pid: pid::Controller::new(FBI_PID_PARAMETERS),
            last_fv_target: None,

            fv_pid: pid::Controller::new(FV_PID_PARAMETERS),
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
                let fd_voltage = 0.9;
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

        let fv_target = self.fbi_pid.update(fbi);
        self.last_fv_target = Some(fv_target);
        self.fv_pid.set_target(fv_target);

        let fv = fv_sample as f32/board::FV_ADC_GAIN;
        let fv_pwm_duty = self.fv_pid.update(fv);
        board::set_fv_pwm(fv_pwm_duty as u16);

        self.last_fv = Some(fv);
        self.last_fbv = Some(fbv_sample as f32/board::FBV_ADC_GAIN);
    }

    pub fn set_emission_target(&mut self, amperes: f32) {
        self.fbi_target = amperes;
        self.fbi_pid.set_target(amperes);
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

    fn emission_ready(&self) -> bool {
        match self.last_fbi {
            None => false,
            Some(last_fbi) => (self.fbi_target - last_fbi).abs()/self.fbi_target < 0.05
        }
    }

    fn bias_ready(&self) -> bool {
        match self.last_fbv {
            None => false,
            Some(last_fbv) => (self.fbv_target - last_fbv).abs() < 1.0
        }
    }

    pub fn reset(&mut self) {
        self.fbi_pid.reset();
        self.fv_pid.reset();
        self.last_fv_target = None;
        self.fbi_buffer_count = 0;
        self.last_fbi = None;
        self.last_fv = None;
        self.last_fbv = None;
    }

    pub fn get_status(&self) -> ControllerStatus {
        ControllerStatus {
            ready: self.emission_ready() & self.bias_ready(),
            fbi: self.last_fbi,
            fv_target: self.last_fv_target,
            fv: self.last_fv,
            fbv: self.last_fbv
        }
    }

}

impl ControllerStatus {
    pub fn debug_print(&self) {
        println!("cathode rdy: {}", self.ready);
        if self.fbi.is_some() {
            println!("emi: {}mA", 1000.0*self.fbi.unwrap());
        }
        if self.fv.is_some() {
            println!("fil: {}V", self.fv.unwrap());
        }
        if self.fbv.is_some() {
            println!("bias: {}V", self.fbv.unwrap());
        }
    }
}
