//! Blanking the screen with `wlr-output-power-management`, and noticing
//! the power button press that wakes it back up.

use smithay_client_toolkit::{
    dispatch2::Dispatch2,
    reexports::{
        client::{Connection, Proxy, QueueHandle},
        protocols_wlr::output_power_management::v1::client::{
            zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
            zwlr_output_power_v1::{Event as OutputPowerEvent, ZwlrOutputPowerV1},
        },
    },
};

use crate::App;

pub(crate) struct OutputPowerManagerData;

impl Dispatch2<ZwlrOutputPowerManagerV1, App> for OutputPowerManagerData {
    fn event(
        &self,
        _state: &mut App,
        _proxy: &ZwlrOutputPowerManagerV1,
        _event: <ZwlrOutputPowerManagerV1 as Proxy>::Event,
        _conn: &Connection,
        _qh: &QueueHandle<App>,
    ) {
    }
}

pub(crate) struct OutputPowerData;

impl Dispatch2<ZwlrOutputPowerV1, App> for OutputPowerData {
    fn event(
        &self,
        state: &mut App,
        proxy: &ZwlrOutputPowerV1,
        event: OutputPowerEvent,
        _conn: &Connection,
        _qh: &QueueHandle<App>,
    ) {
        match event {
            OutputPowerEvent::Mode { mode } => {
                eprintln!("patin-lock: output power mode changed to {mode:?}");
            }
            OutputPowerEvent::Failed => {
                if let Some(index) = state.view_for_power(proxy) {
                    eprintln!(
                        "patin-lock: output power control for output {index} is no longer valid"
                    );
                    state.views[index].power = None;
                }
            }
            _ => {}
        }
    }
}
