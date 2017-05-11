use board;

pub struct Electrometer {
    range: board::ElectrometerRange,
    ic_buffer: [f32; 16],
    ic_buffer_count: usize,
    last_ic: Option<f32>
}

#[derive(Clone, Copy)]
pub struct ElectrometerStatus {
    pub ic: Option<f32>
}

impl Electrometer {
    pub const fn new() -> Electrometer {
        Electrometer {
            range: board::ElectrometerRange::Med,
            ic_buffer: [0.0; 16],
            ic_buffer_count: 0,
            last_ic: None
        }
    }

    pub fn adc_input(&mut self, ic_sample: u16) {
        let gain = match self.range {
            board::ElectrometerRange::Low => board::IC_ADC_GAIN_LOW,
            board::ElectrometerRange::Med => board::IC_ADC_GAIN_MED,
            board::ElectrometerRange::High => board::IC_ADC_GAIN_HIGH
        };
        self.ic_buffer[self.ic_buffer_count] = ((ic_sample as f32) - board::IC_ADC_OFFSET)/gain;
        self.ic_buffer_count += 1;
        if self.ic_buffer_count == self.ic_buffer.len() {
            let mut ic_avg: f32 = 0.0;
            for ic in self.ic_buffer.iter() {
                ic_avg += *ic;
            }
            self.last_ic = Some(ic_avg/(self.ic_buffer.len() as f32));
            self.ic_buffer_count = 0;
        }
    }

    pub fn get_status(&self) -> ElectrometerStatus {
        ElectrometerStatus {
            ic: self.last_ic
        }
    }
}

impl ElectrometerStatus {
    pub fn debug_print(&self) {
        if self.ic.is_some() {
            println!("ion current: {}nA", 1e9*self.ic.unwrap());
        }
    }
}
