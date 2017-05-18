use board;

pub struct Electrometer {
    range: board::ElectrometerRange,
    out_of_range_count: u8,
    ignore_count: u8,
    ic_buffer: [f32; 64],
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
            out_of_range_count: 0,
            ignore_count: 0,
            ic_buffer: [0.0; 64],
            ic_buffer_count: 0,
            last_ic: None
        }
    }

    pub fn adc_input(&mut self, ic_sample: u16) {
        if self.ignore_count > 0 {
            self.ignore_count -= 1;
        } else {
            let mut new_range = if ic_sample > 3100 {
                match self.range {
                    board::ElectrometerRange::Low => Some(board::ElectrometerRange::Med),
                    board::ElectrometerRange::Med => Some(board::ElectrometerRange::High),
                    board::ElectrometerRange::High => None
                }
            } else if ic_sample < 200 {
                match self.range {
                    board::ElectrometerRange::Low => None,
                    board::ElectrometerRange::Med => Some(board::ElectrometerRange::Low),
                    board::ElectrometerRange::High => Some(board::ElectrometerRange::Med)
                }
            } else {
                None
            };

            if new_range.is_some() {
                self.out_of_range_count += 1;
                if self.out_of_range_count < 10 {
                    new_range = None;
                }
            } else {
                self.out_of_range_count = 0;
            }

            if new_range.is_some() {
                self.ignore_count = 20;
                self.ic_buffer_count = 0;
                self.last_ic = None;
                self.range = new_range.unwrap();
                board::set_electrometer_range(self.range);
            } else {
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
            println!("ion: {}nA", 1e9*self.ic.unwrap());
        }
    }
}
