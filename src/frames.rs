#[cfg(feature = "eh1")]
pub(crate) use embedded_can::Frame;
#[cfg(feature = "eh0")]
pub(crate) use embedded_hal::can::Frame;

use crate::interface;
use interface::raw_to_id;
use std::marker::PhantomData;

#[doc = r"! Vehicle CAN frames"]
#[derive(Debug, Default, Copy, Clone)]
pub struct X100 {
    /// Set “minimum current” defined by vehicle
    pub minimum_charge_current: u8,
    /// Lower limit voltage for backup to stop by a charger
    pub minimum_battery_voltage: f32,
    /// Upper limit voltage for backup to stop by a charger
    pub maximum_battery_voltage: f32,
    /// Set fixed value (0x64: 100 %) related to charged rate
    pub constant_of_charging_rate_indication: u8,
}

impl<T> From<&T> for X100
where
    T: for<'a> Frame,
{
    fn from(frame: &T) -> Self {
        let data = data_sanity(frame, 0x100, 8);
        X100 {
            minimum_battery_voltage: u16::from_le_bytes(data[2..=3].try_into().unwrap()) as f32,
            maximum_battery_voltage: u16::from_le_bytes(data[4..=5].try_into().unwrap()) as f32,
            constant_of_charging_rate_indication: data[6],
            minimum_charge_current: data[0],
        }
    }
}

/// Vehicle CAN frame
#[allow(dead_code)]
#[derive(Debug, Default, Copy, Clone)]
pub struct X101 {
    /// Maximum charging time that vehicle permits charger
    max_charging_time_10s_bit: u8,
    /// Maximum charging time that vehicle permits charger
    max_charging_time_1min_bit: u8,
    /// Estimated time until stop of charging
    estimated_charging_time: u8,
    /// Set total capacity of battery
    rated_battery_capacity: f32,
}

impl<T> From<&T> for X101
where
    T: for<'a> Frame,
{
    fn from(frame: &T) -> Self {
        let data = data_sanity(frame, 0x101, 8);
        X101 {
            max_charging_time_10s_bit: data[1],
            max_charging_time_1min_bit: data[2],
            estimated_charging_time: data[3],
            rated_battery_capacity: u16::from_le_bytes(data[5..=6].try_into().unwrap()) as f32,
        }
    }
}

/// Vehicle CAN frame
#[derive(Debug, Default, Copy, Clone)]
pub struct X102 {
    /// CHAdeMO protocol number
    pub control_protocol_number_ev: u8,
    /// Target value of charging voltage
    pub target_battery_voltage: f32,
    /// Charging current request
    pub charging_current_request: u8,
    faults: X102Faults,
    pub status: X102Status,
    /// state of charge of battery
    pub state_of_charge: u8,
}
impl X102 {
    pub fn fault(&self) -> bool {
        self.faults.into()
    }
    pub fn contactors_closed(&self) -> bool {
        !self.status.status_vehicle
    }
    pub fn can_discharge(&self) -> bool {
        self.status.status_discharge_compatible
    }
    /// May mirror kline 102.5.0
    pub fn car_ready(&self) -> bool {
        self.status.status_vehicle_charging
    }
    pub fn can_close_contactors(&self) -> bool {
        !(self.status.status_normal_stop_request
            | self.status.status_charging_system
            | self.status.status_vehicle_shifter_position)
            && self.status.status_vehicle
            && self.status.status_vehicle_charging
            && self.target_battery_voltage > 0.0
    }
    pub fn stop(&self) -> bool {
        false
    }
}

impl<T> From<&T> for X102
where
    T: Frame,
{
    fn from(frame: &T) -> X102 {
        let data = data_sanity(frame, 0x102, 8);
        X102 {
            control_protocol_number_ev: data[0],
            target_battery_voltage: u16::from_le_bytes(data[1..=2].try_into().unwrap()) as f32,
            charging_current_request: data[3],
            faults: From::from(data[4]),
            status: From::from(data[5]),
            state_of_charge: data[6],
        }
    }
}

/// 1 = error, 0 = normal
#[derive(Debug, Default, Copy, Clone)]
pub struct X102Faults {
    /// 102.4.4
    /// - Battery voltage deviation error
    /// - Flag indicating the result of judgment regarding the difference between measured voltage of on- board battery and “Present output voltage” measured by the charger.
    pub fault_battery_voltage_deviation: bool,
    /// 102.4.3 - High battery temperature
    pub fault_high_battery_temperature: bool,
    /// 102.4.2
    /// - Battery current deviation error
    /// — If the EVSE’s output exceeds the maximum charge current continually, the flag shall be changed to 1. The overcurrent threshold shall be set at 10 A (absolute value) or more, and the time threshold shall be set at 5sec or more
    /// — If the EVSE’s input exceeds the range of the maximum discharge current continually, the flag shall be changed to 1. The overcurrent threshold shall be set at 10 A (absolute value) or more and the time threshold shall be set at 5sec or more
    /// — The vehicle charge/discharge enabled and switch (k) shall be turned off at the same time
    /// - Regardless of the condition of the opto-coupler (j), if this flag is 1, it shall be considered as the vehicle’s request to stop charging/discharging, and the EVSE shall move to the stop control.
    pub fault_battery_current_deviation: bool,
    /// 102.4.1
    /// - Status flag indicating the voltage status of on-board battery.
    pub fault_battery_undervoltage: bool,
    /// 102.4.0
    /// - Status flag indicating the voltage status of on-board battery
    /// Regardless of opto-coupler (j) status, the EVSE shall regard this flag as charging termination order from the vehicle if it is equal to 1, and stop charging.
    pub fault_battery_overvoltage: bool,
}
impl From<X102Faults> for bool {
    fn from(val: X102Faults) -> bool {
        val.fault_battery_voltage_deviation
            | val.fault_high_battery_temperature
            | val.fault_battery_current_deviation
            | val.fault_battery_undervoltage
            | val.fault_battery_overvoltage
    }
}

impl From<u8> for X102Faults {
    fn from(value: u8) -> Self {
        Self {
            fault_battery_overvoltage: get_bit(value, 4),
            fault_battery_undervoltage: get_bit(value, 3),
            fault_battery_current_deviation: get_bit(value, 2),
            fault_high_battery_temperature: get_bit(value, 1),
            fault_battery_voltage_deviation: get_bit(value, 0),
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct X102Status {
    /// 102.5.7
    /// - The flag indicating the vehicle is compatible with discharging
    /// The value shall be set from the first time of the CAN communication, and it shall not be updated. However, if it is inevitable to reset the value, e.g. for battery protection, the value is updated from 1 to 0 and only discharging shall be prohibited. — The value indicates the compatibility with the V2H charge/discharge mode (compatible: 1, incompatible: 0)
    pub status_discharge_compatible: bool,
    /// 102.5.4
    /// - Flag used by the vehicle to instruct the EVSE to stop charging control. -
    /// This value shall be updated until initial value of “Charging current request” is set. Do not update this value after initial value transmission.
    pub status_normal_stop_request: bool,
    /// 102.5.3
    ///  - Flag indicating the OPEN/CLOSE status of EV contactors and the result of vehicle contactor welding detection.
    /// Set the flag to 0 when the vehicle relay is closed, and set as 1 after the termination of welding detection. - Set the flag to 0 when the vehicle relay is closed, and set as 1 after the termination of welding detection.
    pub status_vehicle: bool, // true EV contactors open
    /// 102.5.2
    /// - Flag indicating the presence of the malfunction originated in the vehicle among the malfunctions detected by the vehicle.
    /// Update as needed, and hold “1” after the malfunction is determined. — Regardless of the condition of the opto-coupler (j), if this flag is 0, it shall be considered as the vehicle's request to stop charging/discharging, and the EVSE shall move to the stop control.
    pub status_charging_system: bool, // false = ok / true = fault
    /// 102.5.1
    /// - Status flag indicating the shift lever position
    /// — Set this flag to 0 when the shift lever is in “parking” position. Set to 1 when it is in other position. — Turn the switch (k) OFF if the shift position is changed except “parking” during charging.
    pub status_vehicle_shifter_position: bool, // false = ok
    /// 102.5.0
    /// - Flag indicating charging/dischar ging permission status of the vehicle.
    /// Charging/discharging enabled: 1, charging/discharging disabled: 0
    /// — After CAN communication starts and the vehicle sends the EVSE data required for prior to a start of charging/discharging, change the flag 0 to 1. — Change this flag 1 to 0 when the vehicle sends the “charging/discharging stop” notification to the EVSE. Regardless of the condition of the opto-coupler (j), if this flag is 0, it shall be considered as the vehicle's request to stop charging/discharging, and the EVSE shall move to the stop control.— When this flag is 0, the insulation test shall not be conducted.
    pub status_vehicle_charging: bool,
}
impl std::fmt::Display for X102Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "102.5.0:{} 1:{} 2:{} 3:{} 4:{} 7:{}",
            self.status_vehicle_charging as u8,
            self.status_vehicle_shifter_position as u8,
            self.status_charging_system as u8,
            self.status_vehicle as u8,
            self.status_normal_stop_request as u8,
            self.status_discharge_compatible as u8,
        )
    }
}
impl From<u8> for X102Status {
    fn from(val: u8) -> Self {
        Self {
            status_discharge_compatible: get_bit(val, 7),
            status_normal_stop_request: get_bit(val, 4),
            status_vehicle: get_bit(val, 3),
            status_charging_system: get_bit(val, 2),
            status_vehicle_shifter_position: get_bit(val, 1),
            status_vehicle_charging: get_bit(val, 0),
        }
    }
}
impl From<X102Status> for u8 {
    fn from(val: X102Status) -> Self {
        let mut result: u8 = 0;

        result |= (val.status_discharge_compatible as u8) << 7;
        result |= (val.status_normal_stop_request as u8) << 4;
        result |= (val.status_vehicle as u8) << 3;
        result |= (val.status_charging_system as u8) << 2;
        result |= (val.status_vehicle_shifter_position as u8) << 1;
        result |= val.status_vehicle_charging as u8;

        result
    }
}

/// EVSE CAN frame
#[derive(Debug, Copy, Clone)]
pub struct X108<T>
where
    T: Frame,
{
    /// 108.3 - Current that the EVSE can output at present.
    ///
    /// This value shall be set from the initial CAN communication. The initial value shall be the maximum current that can be output by the EVSE, and during the charging/discharging, the value shall be updated from time to time as the current which can be output by the EVSE.
    /// The smaller value between this value and the “maximum charge current” shall be set as the target charge current.
    pub available_output_current: u8,
    /// 108.1-2 - Maximum output voltage value of the EVSE.
    ///
    /// Set the number from initial CAN data transmission and do not update it.
    /// If the EVSE receives “target battery voltage” exceeding this value from the vehicle, regard this situation as “Battery incompatible” and shift to charge termination process.
    pub avaible_output_voltage: u16,
    /// 108.0 - Identifier indicating characteristic of output circuit of EVSE which corresponds to welding detection of EV contactor.
    pub welding_detection: u8,
    /// 108.4-5 - Judgmental voltage value to stop charging process for on-board battery protection.
    ///
    /// This flag may be updated until the initial value of charging current request is sent from the vehicle.
    /// — The EVSE shall compare vehicle CAN “maximum battery voltage” with charger CAN “available output voltage,” set the lower value to this value. — When circuit voltage reaches to this value, the EVSE stops charging output.
    pub threshold_voltage: u16,
    phantom: PhantomData<T>,
}

impl<T> X108<T>
where
    T: Frame,
{
    pub fn to_can(&self) -> Option<T> {
        let aov = self.avaible_output_voltage.to_le_bytes();
        let tv = self.threshold_voltage.to_le_bytes();
        T::new(
            raw_to_id(0x108),
            &[
                self.welding_detection,
                aov[0],
                aov[1],
                self.available_output_current,
                tv[0],
                tv[1],
                0,
                0,
            ],
        )
    }
    pub fn new(
        available_output_current: u8,
        avaible_output_voltage: u16,
        welding_detection: bool,
        threshold_voltage: u16,
    ) -> Self {
        Self {
            available_output_current,
            avaible_output_voltage,
            welding_detection: welding_detection.into(),
            threshold_voltage,
            phantom: PhantomData,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct X109Status {
    /// 109.5.5 - Set this flag to 1 before charging (e.g., initial value and during insulation test).
    ///
    /// Change this flag to 0 from 1 after shifting to the start of charging control. Also, both the timing that the “charging stop control (H’109.5.5)” is changed to 0 from 1 and the timing that the “charger status (H’109.5.0)” is changed to 1 from 0 shall be in an exclusive relation. Set 1 from 0 to this flag in case the charging sequence shifts to stop process (including a state of stop process).
    pub status_charger_stop_control: bool,
    /// 109.5.4 - Error flag indicating vehicle error or charger error.
    ///
    /// Charger shall detect error and shall shift to error stop process in case this flag is set to 1.
    pub fault_charging_system_malfunction: bool,
    /// 109.5.3 -Error flag indicating “available output voltage” of charger which is not suitable for charging to traction battery.
    ///
    /// Set 1 to this flag in case “target battery voltage (H’102.1, H’102.2)” of vehicle exceeds “available output voltage (H’108.1, H’108.2)” or “Minimum battery voltage (H’100.2, H’ 100.3)” of vehicle is below “output voltage lower limit." Charger shall detect error and shall shift to error stop process in case this flag is set to 1.
    pub fault_battery_incompatibility: bool,
    /// 109.5.2 - Status flag indicating a state in which voltage can be applied from charger or a state in which output charging is permitted.
    ///
    /// Set 1 to this flag when vehicle permits charger to charge and/or voltage in output circuit exceeds 10 V. Set 0 to this flag when vehicle prohibits charger to charge and/or voltage in output circuit is less than or equal to 10 V.
    pub status_vehicle_connector_lock: bool,
    /// 109.5.1 - Error flag indicating charger’s error detected by charger
    ///
    /// Charger shall detect error and shall shift to error stop process in case this flag is set to 1.
    pub fault_station_malfunction: bool,
    /// 109.5.0 - Status flag indicating charging
    ///
    /// Set 0 to this flag before charging (e.g., initial value, during insulation test) and at the end of the charging (shifting to stop process and charging current decreases less than or equal to 5 A). Set 1 to this flag during charging
    pub status_station: bool,
}
impl std::fmt::Display for X109Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "109.5.0:{} 1:{} 2:{} 3:{} 4:{} 5:{}",
            self.status_station as u8,
            self.fault_station_malfunction as u8,
            self.status_vehicle_connector_lock as u8,
            self.fault_battery_incompatibility as u8,
            self.fault_charging_system_malfunction as u8,
            self.status_charger_stop_control as u8
        )
    }
}
impl From<X109Status> for u8 {
    fn from(val: X109Status) -> Self {
        let mut result = 0u8;
        result |= (val.status_charger_stop_control as u8) << 5;
        result |= (val.fault_charging_system_malfunction as u8) << 4;
        result |= (val.fault_battery_incompatibility as u8) << 3;
        result |= (val.status_vehicle_connector_lock as u8) << 2;
        result |= (val.fault_station_malfunction as u8) << 1;
        result |= val.status_station as u8;
        result
    }
}
impl From<u8> for X109Status {
    fn from(value: u8) -> Self {
        X109Status {
            status_charger_stop_control: get_bit(value, 5),
            fault_charging_system_malfunction: get_bit(value, 4),
            fault_battery_incompatibility: get_bit(value, 3),
            status_vehicle_connector_lock: get_bit(value, 2),
            fault_station_malfunction: get_bit(value, 1),
            status_station: get_bit(value, 0),
        }
    }
}
/// EVSE CAN frame
#[derive(Default, Debug, Clone, Copy)]
pub struct X109<T> {
    pub status: X109Status,
    control_protocol_number_qc: u8,
    pub output_voltage: f32,
    pub output_current: u8,
    discharge_compatitiblity: bool,
    pub remaining_charging_time_10s_bit: u8,
    pub remaining_charging_time_1min_bit: u8,
    phantom: PhantomData<T>,
}

impl<T: Frame> X109<T> {
    pub fn to_can(&self) -> Option<T> {
        let mut result = [0u8; 8];

        result[0] = self.control_protocol_number_qc;
        let voltage_bytes: [u8; 2] = ((self.output_voltage) as u16).to_le_bytes();
        result[1..=2].copy_from_slice(&voltage_bytes);
        result[3] = self.output_current;
        result[4] = self.discharge_compatitiblity.into(); // EVSE discharge compatitbility flag
        result[5] = self.status.into();
        result[6] = self.remaining_charging_time_10s_bit;
        result[7] = self.remaining_charging_time_1min_bit;
        let id = raw_to_id(0x109);
        T::new(id, &result)
    }
    pub fn new(control_protocol_number_qc: u8, discharge_compatitiblity: bool) -> Self {
        let status = X109Status {
            status_charger_stop_control: true,
            ..Default::default()
        };
        Self {
            control_protocol_number_qc,
            discharge_compatitiblity,
            remaining_charging_time_10s_bit: 255,
            remaining_charging_time_1min_bit: 255,
            status,
            output_voltage: 0.0,
            output_current: 0,
            phantom: PhantomData,
        }
    }
}

impl<T> From<&T> for X109<T>
where
    T: Frame,
{
    fn from(frame: &T) -> X109<T> {
        let data = data_sanity::<T>(frame, 0x109, 8);
        Self {
            control_protocol_number_qc: data[0],
            output_voltage: u16::from_le_bytes([data[1], data[2]]) as f32,
            output_current: data[3],
            discharge_compatitiblity: data[4] == 1,
            status: data[5].into(),
            remaining_charging_time_10s_bit: data[6],
            remaining_charging_time_1min_bit: data[7],
            phantom: PhantomData,
        }
    }
}

// Vehicle can frame
#[derive(Default, Debug, Clone, Copy)]
pub struct X200 {
    /// Maximum discharge current that the vehicle permits to the EVSE.
    ///
    /// This value shall be set according to the vehicle’s battery condition in consideration of the following conditions. The initial value shall be set at 0 and the value shall be constantly updated (only when it is inevitable, e.g., for battery protection).
    /// At the time of the Charge/discharge mode, discharging shall be implemented with this value as the upper limit. — There are vehicles of the models before the V2H guideline 1.1 whose initial value is not set at 0. The control error shall be avoided by masking the initial value etc. — If EVSE has a bigger this value than Available input current, it does not use stopping judgment.
    pub maximum_discharge_current: u8,
    /// Minimum voltage that the vehicle can discharge.
    ///
    /// This value can be updated until the switch (k) is turned off.
    ///  Once this value is set, it shall not be updated.
    pub minimum_discharge_voltage: u16,
    /// Minimum battery capacity with which the vehicle permits discharging.
    ///
    /// This value shall be set as the minimum discharge voltage of the vehicle battery.
    /// If this value is not used, 0x00 shall be set.
    /// When the EVSE reaches this value, the EVSE prohibits only discharge. (But the EVSE can continue charge.) However, in case of the vehicles before the V2H guideline 1.0, the unit of this value is kWh.
    ///
    /// Using the next expression, the EVSE converts a unit into %. Minimum discharging rate for charging [%] = Minimum remaining battery capacity for charging [kWh] ÷ Total battery capacity [kWh]×100 [%] In addition, the EVSE cuts off a decimal and applies a unit conversion result. — The EVSE shall not be used until the switch (k) is turned on.
    pub minimum_battery_discharge_level: u8,
    /// Maximum battery capacity with which the vehicle permits charging.
    ///
    /// This value shall be set as the maximum charging capacity of the vehicle battery. — If this value is not used, 0x00 shall be set.
    /// When the EVSE reaches this value, the EVSE prohibits only charge. (But the EVSE can continue discharge.)
    ///
    /// However, in case of the vehicles before the V2H guideline 1.0, the unit of this value is kWh. Using the next expression, the EVSE converts a unit into %. Maximum charging rate for charging [%] = Maximum remaining battery capacity for charging [kWh] ÷ Total battery capacity [kWh] × 100 [%]. In addition, the EVSE cuts off a decimal and applies a unit conversion result. — When the EVSE receives 0, it shall not be used with the assumption that the value is not set. — The EVSE shall not be used until the switch (k) is turned on.
    pub max_remaining_capacity_for_charging: u8,
    // phantom: PhantomData<T>,
}

impl<T> From<&T> for X200
where
    T: Frame,
{
    fn from(frame: &T) -> X200 {
        let data = data_sanity(frame, 0x200, 8);
        Self {
            maximum_discharge_current: 255 - data[0],
            minimum_discharge_voltage: u16::from_le_bytes(data[4..=5].try_into().unwrap()),
            minimum_battery_discharge_level: 255 - data[6],
            max_remaining_capacity_for_charging: data[7],
            // phantom: PhantomData,
        }
    }
}

/// EVSE V2x

#[derive(Debug, Clone, Copy)]
pub struct X208<T>
where
    T: Frame,
{
    /// The circuit current measured by the EVSE.
    pub discharge_current: u8,
    /// The minimum voltage with which the EVSE can operate.
    input_voltage: u16,
    /// The current with which the EVSE stops discharging in order to protect the circuit
    input_current: u8,
    /// The voltage with which the EVSE shall stop when the vehicle cannot stop at the minimum discharge voltage of the vehicle system due to a fault.
    lower_threshold_voltage: u16,
    phantom: PhantomData<T>,
}
impl<T> X208<T>
where
    T: Frame,
{
    pub fn to_can(&self) -> Option<T> {
        let mut data = [0u8; 8];

        data[0] = 0xff - self.discharge_current;
        [data[1], data[2]] = self.input_voltage.to_le_bytes();
        data[3] = 0xff - self.input_current;
        [data[6], data[7]] = self.lower_threshold_voltage.to_le_bytes();
        let id = raw_to_id(0x208);
        T::new(id, &data)
    }
    /// positive is discharge - discharge_current is real time, input_* are adjustable limits
    pub fn new(
        discharge_current: u8,
        input_voltage: u16,
        input_current: u8,
        lower_threshold_voltage: u16,
    ) -> Self {
        Self {
            discharge_current,
            input_voltage,
            input_current,
            lower_threshold_voltage,
            phantom: PhantomData,
        }
    }

    pub fn get_discharge_current(&self) -> u8 {
        self.discharge_current
    }
    pub fn set_discharge_current(&mut self, amps: impl Into<u8>) {
        self.discharge_current = amps.into();
    }
    pub fn get_input_voltage(&self) -> u16 {
        self.input_voltage
    }

    /// Discharge limit
    pub fn get_input_current(&self) -> u8 {
        self.input_current
    }

    pub fn set_input_voltage(&mut self) -> u16 {
        self.input_voltage
    }

    /// Discharge limit
    pub fn set_input_current(&mut self, amps: impl Into<u8>) {
        self.input_current = amps.into();
    }
    pub fn get_lower_threshold_voltage(&self) -> u16 {
        self.lower_threshold_voltage
    }
}

impl<T> From<&T> for X208<T>
where
    T: Frame,
{
    fn from(frame: &T) -> Self {
        let data = data_sanity(frame, 0x208, 8);
        X208 {
            discharge_current: 255 - data[0],
            input_voltage: u16::from_le_bytes(data[1..=2].try_into().unwrap()),
            input_current: 255 - data[3],
            lower_threshold_voltage: u16::from_le_bytes(data[6..=7].try_into().unwrap()),
            phantom: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct X209<T> {
    /// Charge/dis charge sequence control number
    sequence: u8,
    /// Remaining discharging time
    pub remaing_discharge_time: u16,
    phantom: PhantomData<T>,
}

impl<T> X209<T>
where
    T: Frame,
{
    pub fn to_can(&self) -> Option<T> {
        let mut data = [0u8; 8];

        data[0] = self.sequence;
        [data[1], data[2]] = self.remaing_discharge_time.to_le_bytes();
        let id = raw_to_id(0x209);
        T::new(id, &data)
    }
    pub fn new(sequence: u8, remaing_discharge_time: u16) -> Self {
        Self {
            sequence,
            remaing_discharge_time,
            phantom: PhantomData,
        }
    }
}

impl<T> From<&T> for X209<T>
where
    T: Frame,
{
    fn from(frame: &T) -> Self {
        let data = data_sanity(frame, 0x209, 8);
        Self {
            sequence: data[0],
            remaing_discharge_time: u16::from_le_bytes(data[1..=2].try_into().unwrap()),
            phantom: PhantomData,
        }
    }
}

#[inline]
fn get_bit(byte: u8, position: u8) -> bool {
    (byte & (1 << position)) != 0
}

#[inline]
fn data_sanity<T>(frame: &T, id: u32, dlc: usize) -> &[u8]
where
    T: Frame,
{
    assert!(
        frame.id() == raw_to_id(id as u16),
        "CANFrame decoder error: Incorrect ID can frame"
    );
    assert!(
        frame.data().len() == dlc,
        "CANFrame decoder error: DLC for can frame is not 8"
    );
    frame.data()
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::ChademoCanFrame;
    #[test]
    fn x109_test() {
        let id = raw_to_id(0x109);
        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x00, 0x00, 0x00, 0x01, 0x20, 0x00, 0x00].as_slice(),
        )
        .unwrap();
        let x109 = X109::from(&frame);
        assert!(!x109.status.status_vehicle_connector_lock);
        assert!(x109.status.status_charger_stop_control);

        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x80, 0x01, 0x00, 0x01, 0x24, 0x00, 0x00].as_slice(),
        )
        .unwrap();
        let x109 = X109::from(&frame);
        assert!(x109.status.status_charger_stop_control);

        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x80, 0x01, 0x00, 0x01, 0x05, 0x00, 0x00].as_slice(),
        )
        .unwrap();
        let x109 = X109::from(&frame);
        assert!(!x109.status.status_charger_stop_control);
        assert!(x109.status.status_station);
    }
    #[test]
    fn x102_test() {
        let id = raw_to_id(0x102);
        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x9A, 0x01, 0x00, 0x00, 0xC8, 0x56, 0x00].as_slice(),
        )
        .unwrap();
        let x102: X102 = X102::from(&frame);
        println!("{}", x102.status);
        assert!(!x102.contactors_closed());

        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x9A, 0x01, 0x00, 0x00, 0xC9, 0x56, 0x00].as_slice(),
        )
        .unwrap();
        let x102: X102 = X102::from(&frame);
        assert!(x102.can_close_contactors());
        println!("{}", x102.status);

        let frame = ChademoCanFrame::new(
            id,
            [0x02, 0x9A, 0x01, 0x00, 0x00, 0xC1, 0x56, 0x00].as_slice(),
        )
        .unwrap();
        let x102 = X102::from(&frame);
        assert!(x102.contactors_closed());
    }
}
