#![feature(error_in_core)]
/// Notes from:
/// IEEE Std 2030.1.1-2021
/// IEEE Standard for Technical Specifications of a DC Quick Charger for Use with Electric Vehicles
use frames::*;
use interface::standard_id_to_raw;

mod error;
mod frames;
mod interface;

#[derive(Clone, Debug)]
pub struct Chademo<T>
where
    T: Frame,
{
    pub x100: X100,
    pub x101: X101,
    pub x102: X102,
    pub x108: X108<T>,
    pub x109: X109<T>,
    pub x200: X200,
    pub x208: X208<T>,
    pub x209: X209<T>,
}

impl<T> std::fmt::Display for Chademo<T>
where
    T: Frame,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let x102 = format!("{}", self.x102_status());
        let x109 = format!("{}", self.x109_status());
        write!(
            f,
            "x102: status {}\nx109: status {}\nV2x: Max Dis: (-{}A {}%) Chg: {}A )",
            x102,
            x109,
            self.requested_discharging_amps(),
            self.max_remaining_capacity_for_charging(),
            self.requested_charging_amps(),
        )
    }
}

impl<T> Chademo<T>
where
    T: Frame,
{
    pub fn new(max_amps: u8) -> Self {
        Self {
            //EV decode
            x100: X100::default(),
            x101: X101::default(),
            x102: X102::default(),
            x200: X200::default(),
            //EVSE encode
            x109: X109::new(2, true),
            x108: X108::new(max_amps, 500, true, 435).into(),
            x208: X208::new(0, 500, max_amps, 250),
            x209: X209::new(2, 0),
        }
    }

    pub fn decode(&mut self, frame: T) -> Result<(), error::ChademoError> {
        Ok(match standard_id_to_raw(frame.id())? {
            0x100 => self.x100 = X100::from(&frame),
            0x101 => self.x101 = X101::from(&frame),
            0x102 => self.x102 = X102::from(&frame),
            0x200 => self.x200 = X200::from(&frame),
            bad_id => return Err(error::ChademoError::DecodeBadId(bad_id)),
        })
    }
    /// Flag to EV that charge has been cancelled
    /// Sets 109.5.5 high
    pub fn request_stop_charge(&mut self) {
        // change status of x109
        self.x109.status.status_charger_stop_control = true;
    }
    pub fn x102_status(&self) -> X102Status {
        self.x102.status
    }
    pub fn x109_status(&self) -> X109Status {
        self.x109.status
    }
    pub fn tx_frames(&self) -> [Option<T>; 4] {
        [
            self.x108.to_can(),
            self.x109.to_can(),
            self.x208.to_can(),
            self.x209.to_can(),
        ]
    }
    pub fn update_dynamic_charge_limits(&mut self, amps: impl Into<f32>) {
        let amps: f32 = amps.into();
        match amps.is_sign_negative() {
            true => self.set_max_discharge_amps((-1.0 * amps) as u8),
            false => self.set_max_charge_amps(amps as u8),
        }
    }
    pub fn output_volts(&self) -> &f32 {
        &self.x109.output_voltage
    }
    fn set_max_charge_amps(&mut self, amps: impl Into<u8>) {
        self.x109.output_current = amps.into();
    }
    fn set_max_discharge_amps(&mut self, amps: impl Into<u8>) {
        self.x208.set_input_current(amps.into());
    }
    pub fn soc(&self) -> &u8 {
        &self.x102.state_of_charge
    }
    pub fn requested_charging_amps(&self) -> f32 {
        self.x102.charging_current_request as f32
    }
    pub fn requested_discharging_amps(&self) -> f32 {
        self.x200.maximum_discharge_current as f32
    }
    pub fn max_remaining_capacity_for_charging(&self) -> f32 {
        self.x200.max_remaining_capacity_for_charging as f32
    }

    pub fn status_vehicle_contactors(&self) -> bool {
        self.x102.status.status_vehicle
    }

    pub fn fault(&self) -> bool {
        self.x102.fault().into()
    }

    pub fn target_voltage(&self) -> &f32 {
        &self.x102.target_battery_voltage
    }

    pub fn charge_start(&mut self) {
        self.x109.status.status_charger_stop_control = false;
        self.x109.status.status_station = true;
        self.x109.remaining_charging_time_10s_bit = 255;
        self.x109.remaining_charging_time_1min_bit = 60;
    }
    pub fn charge_stop(&mut self) {
        self.x109.output_voltage = 0.0;
        self.x109.output_current = 0;
        self.x109.remaining_charging_time_10s_bit = 0;
        self.x109.remaining_charging_time_1min_bit = 0;
        self.x109.status.fault_battery_incompatibility = false;
        self.x109.status.fault_charging_system_malfunction = false;
        self.x109.status.fault_station_malfunction = false;
    }
    pub fn plug_lock(&mut self, state: bool) {
        self.x109.status.status_vehicle_connector_lock = state;
    }
    pub fn status_vehicle_charging(&self) -> bool {
        self.x102.status.status_vehicle_charging
    }
    pub fn status_vehicle_ok(&self) -> bool {
        !self.x102.status.status_vehicle
    }
    pub fn charging_stop_control_set(&mut self) {
        self.x109.status.status_charger_stop_control = false
    }
    pub fn charging_stop_control_release(&mut self) {
        self.x109.status.status_charger_stop_control = true
    }
}

#[cfg(test)]
mod test {
    use embedded_can::Frame as CANFrame;
    use frames::X109;

    use crate::interface::{raw_to_id, ChademoCanFrame};

    use super::*;
    #[test]
    fn soc_test() {
        let frame = ChademoCanFrame::new(
            raw_to_id(0x102),
            [0x2, 0x9A, 0x1, 0x0E, 0x0, 0xC1, 0x56, 0x0].as_slice(),
        )
        .unwrap();

        let mut chademo = Chademo::new(15);
        chademo.x109 = X109::<ChademoCanFrame>::new(2, true);
        chademo.x102 = X102::from(&frame);
        assert_eq!(chademo.soc(), &86)
    }
    #[test]
    fn x208_test() {
        let y = X208::<ChademoCanFrame>::new(1, 500, 16, 250);
        println!(
            "{} {} {} {}",
            y.get_discharge_current(),
            y.get_input_voltage(),
            y.get_input_current(),
            y.get_lower_threshold_voltage()
        );
        assert!(y.get_discharge_current() == 1);
        assert!(y.get_input_voltage() == 500);
        assert!(y.get_input_current() == 16);
        assert!(y.get_lower_threshold_voltage() == 250);
        let cf: ChademoCanFrame = y.to_can().unwrap();
        assert!(cf.data()[0] == 0xff - 1);
        assert!(cf.data()[3] == 0xff - 16);

        let y = X208::<ChademoCanFrame>::from(&cf);
        println!(
            "{} {} {} {}",
            y.get_discharge_current(),
            y.get_input_voltage(),
            y.get_input_current(),
            y.get_lower_threshold_voltage()
        );
        assert!(y.get_discharge_current() == 1);
        assert!(y.get_input_voltage() == 500);
        assert!(y.get_input_current() == 16);
        assert!(y.get_lower_threshold_voltage() == 250);
    }
}
