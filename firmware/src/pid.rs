use core::f32;
use core::num::Float;

#[derive(Clone, Copy)]
pub struct Parameters {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    pub output_min: f32,
    pub output_max: f32,
    pub integral_min: f32,
    pub integral_max: f32
}

pub struct Controller {
    parameters: Parameters,
    target: f32,
    integral: f32,
    last_input: f32
}

impl Controller {
    pub const fn new(parameters: Parameters) -> Controller {
        Controller {
            parameters: parameters,
            target: 0.0,
            last_input: f32::NAN,
            integral: 0.0
        }
    }

    pub fn update(&mut self, input: f32) -> f32 {
        let error = self.target - input;

        let p = self.parameters.kp * error;

        self.integral += error;
        if self.integral < self.parameters.integral_min {
            self.integral = self.parameters.integral_min;
        }
        if self.integral > self.parameters.integral_max {
            self.integral = self.parameters.integral_max;
        }
        let i = self.parameters.ki * self.integral;

        let d = if self.last_input.is_nan() {
            0.0
        } else {
            self.parameters.kd * (self.last_input - input)
        };
        self.last_input = input;

        let mut output = p + i + d;
        if output < self.parameters.output_min {
            output = self.parameters.output_min;
        }
        if output > self.parameters.output_max {
            output = self.parameters.output_max;
        }
        output
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.last_input = f32::NAN;
    }
}
